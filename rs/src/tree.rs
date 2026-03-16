//! Hash tree construction.
//!
//! `hash_leaf` hashes leaf data into a chaining value.
//! `hash_node` combines two child chaining values into a parent node.

use crate::encoding::{bytes_to_cv, hash_to_bytes};
use crate::field::Goldilocks;
use crate::params::{self, CHUNK_SIZE, MAX_TREE_DEPTH, OUTPUT_ELEMENTS, RATE, WIDTH};
use crate::sponge::Hash;

/// Flags encoded in the capacity for tree operations.
const FLAG_ROOT: u64 = 1 << 0;
const FLAG_PARENT: u64 = 1 << 1;
const FLAG_CHUNK: u64 = 1 << 2;

/// Capacity index for chunk counter (position in the file).
const CAPACITY_COUNTER_IDX: usize = RATE; // state[8]

/// Capacity index for tree flags.
const CAPACITY_FLAGS_IDX: usize = RATE + 1; // state[9]

/// Capacity index for namespace lower bound (NMT only).
const CAPACITY_NS_MIN_IDX: usize = RATE + 4; // state[12]

/// Capacity index for namespace upper bound (NMT only).
const CAPACITY_NS_MAX_IDX: usize = RATE + 5; // state[13]

/// Compute the chaining value for a leaf chunk.
///
/// The `counter` is the chunk's position index within the file (0-based),
/// used for ordering in tree construction. The `is_root` flag
/// domain-separates root finalization (single-chunk inputs) from interior
/// finalization.
pub fn hash_leaf(chunk: &[u8], counter: u64, is_root: bool) -> Hash {
    let mut hasher = crate::sponge::Hasher::new();
    hasher.update(chunk);
    let base_hash = hasher.finalize();

    // Re-derive with flags and counter via single-permutation.
    let base_elems = bytes_to_cv(base_hash.as_bytes());
    let mut state = [Goldilocks::new(0); WIDTH];
    state[..OUTPUT_ELEMENTS].copy_from_slice(&base_elems);

    let mut flags = FLAG_CHUNK;
    if is_root {
        flags |= FLAG_ROOT;
    }
    state[CAPACITY_COUNTER_IDX] = Goldilocks::new(counter);
    state[CAPACITY_FLAGS_IDX] = Goldilocks::new(flags);

    params::permute(&mut state);

    let output: [Goldilocks; OUTPUT_ELEMENTS] = state[..OUTPUT_ELEMENTS].try_into().unwrap();
    Hash::from_bytes(hash_to_bytes(&output))
}

/// Combine two child chaining values into a parent chaining value.
///
/// With Hemera parameters (output=8 elements, rate=8), each child hash is 8 elements.
/// We absorb left (8 elements) then right (8 elements) via two sponge absorptions,
/// with flags set in the capacity before the first permutation.
///
/// The `is_root` flag domain-separates the tree root from interior nodes.
pub fn hash_node(left: &Hash, right: &Hash, is_root: bool) -> Hash {
    let left_elems = bytes_to_cv(left.as_bytes());
    let right_elems = bytes_to_cv(right.as_bytes());

    let mut state = [Goldilocks::new(0); WIDTH];

    // Set flags in capacity before absorbing.
    let mut flags = FLAG_PARENT;
    if is_root {
        flags |= FLAG_ROOT;
    }
    state[CAPACITY_FLAGS_IDX] = Goldilocks::new(flags);

    // Absorb left child (8 elements = full rate block, Goldilocks field addition).
    for i in 0..RATE {
        state[i] = state[i] + left_elems[i];
    }
    params::permute(&mut state);

    // Absorb right child (8 elements = full rate block, Goldilocks field addition).
    for i in 0..RATE {
        state[i] = state[i] + right_elems[i];
    }
    params::permute(&mut state);

    let output: [Goldilocks; OUTPUT_ELEMENTS] = state[..OUTPUT_ELEMENTS].try_into().unwrap();
    Hash::from_bytes(hash_to_bytes(&output))
}

/// Combine two child chaining values into a namespace-aware parent.
///
/// Extends `hash_node` with namespace bounds committed in capacity:
/// `state[12] = ns_min`, `state[13] = ns_max`. When both are zero this
/// reduces to `hash_node`. See spec §4.6.4.
pub fn hash_node_nmt(
    left: &Hash,
    right: &Hash,
    ns_min: u64,
    ns_max: u64,
    is_root: bool,
) -> Hash {
    let left_elems = bytes_to_cv(left.as_bytes());
    let right_elems = bytes_to_cv(right.as_bytes());

    let mut state = [Goldilocks::new(0); WIDTH];

    let mut flags = FLAG_PARENT;
    if is_root {
        flags |= FLAG_ROOT;
    }
    state[CAPACITY_FLAGS_IDX] = Goldilocks::new(flags);
    state[CAPACITY_NS_MIN_IDX] = Goldilocks::new(ns_min);
    state[CAPACITY_NS_MAX_IDX] = Goldilocks::new(ns_max);

    for i in 0..RATE {
        state[i] = state[i] + left_elems[i];
    }
    params::permute(&mut state);

    for i in 0..RATE {
        state[i] = state[i] + right_elems[i];
    }
    params::permute(&mut state);

    let output: [Goldilocks; OUTPUT_ELEMENTS] = state[..OUTPUT_ELEMENTS].try_into().unwrap();
    Hash::from_bytes(hash_to_bytes(&output))
}

/// Hash arbitrary-length content into a single root hash.
///
/// Splits `data` into `CHUNK_SIZE`-byte chunks and builds a left-balanced
/// binary Merkle tree. See spec §4.6.1–§4.6.5.
///
/// - Single chunk (≤ 4096 bytes): `hash_leaf(data, 0, is_root=true)`
/// - Multiple chunks: left-balanced tree via `hash_leaf` + `hash_node`
///
/// Uses a fixed-size stack (no heap allocation). The stack-based merge
/// follows the BLAKE3 pattern: after adding chunk i (0-indexed),
/// merge while `(i+1)` has trailing zero bits in the left-balanced split.
pub fn root_hash(data: &[u8]) -> Hash {
    if data.is_empty() {
        return hash_leaf(data, 0, true);
    }

    let n = (data.len() + CHUNK_SIZE - 1) / CHUNK_SIZE;

    if n == 1 {
        return hash_leaf(data, 0, true);
    }

    merge_range(data, 0, n, true)
}

/// Hash arbitrary-length content with a progress callback.
///
/// `progress` receives `(completed, total)` where total = 2n−1 (n leaves + n−1 nodes).
#[allow(unknown_lints, rs_no_vec, rs_no_box)]
pub fn root_hash_with_progress(data: &[u8], progress: impl Fn(usize, usize)) -> Hash {
    if data.is_empty() {
        return hash_leaf(data, 0, true);
    }
    let n = (data.len() + CHUNK_SIZE - 1) / CHUNK_SIZE;
    if n == 1 {
        return hash_leaf(data, 0, true);
    }
    let total = 2 * n - 1;
    let done = core::cell::Cell::new(0usize);
    let tick = |d: &core::cell::Cell<usize>| {
        let v = d.get() + 1;
        d.set(v);
        progress(v, total);
    };
    merge_range_progress(data, 0, n, true, &done, &tick)
}

/// Recursive merge with progress counter.
#[allow(unknown_lints, rs_no_vec, rs_no_box)]
fn merge_range_progress(
    data: &[u8],
    offset: usize,
    count: usize,
    is_root: bool,
    done: &core::cell::Cell<usize>,
    tick: &impl Fn(&core::cell::Cell<usize>),
) -> Hash {
    debug_assert!(count > 0);
    let n_total = if data.is_empty() { 1 } else { (data.len() + CHUNK_SIZE - 1) / CHUNK_SIZE };

    if count == 1 {
        let start = offset * CHUNK_SIZE;
        let end = (start + CHUNK_SIZE).min(data.len());
        let h = hash_leaf(&data[start..end], offset as u64, n_total == 1);
        tick(done);
        return h;
    }

    let split = 1 << (usize::BITS - (count - 1).leading_zeros() - 1);
    let left = merge_range_progress(data, offset, split, false, done, tick);
    let right = merge_range_progress(data, offset + split, count - split, false, done, tick);
    let h = hash_node(&left, &right, is_root);
    tick(done);
    h
}

/// Recursively merge chaining values for chunks `[offset..offset+count)`,
/// computing leaf hashes on demand from the data slice.
pub(crate) fn merge_range(data: &[u8], offset: usize, count: usize, is_root: bool) -> Hash {
    debug_assert!(count > 0);
    let n_total = if data.is_empty() { 1 } else { (data.len() + CHUNK_SIZE - 1) / CHUNK_SIZE };

    if count == 1 {
        let start = offset * CHUNK_SIZE;
        let end = (start + CHUNK_SIZE).min(data.len());
        return hash_leaf(&data[start..end], offset as u64, n_total == 1);
    }

    // Left subtree is a complete binary tree: split = 2^(ceil(log2(count)) - 1)
    let split = 1 << (usize::BITS - (count - 1).leading_zeros() - 1);
    let left = merge_range(data, offset, split, false);
    let right = merge_range(data, offset + split, count - split, false);
    hash_node(&left, &right, is_root)
}

// ── Inclusion proofs ─────────────────────────────────────────────

/// A sibling entry in an inclusion proof path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Sibling {
    /// The sibling is on the left; the target node is on the right.
    Left(Hash),
    /// The sibling is on the right; the target node is on the left.
    Right(Hash),
}

/// Merkle inclusion proof for a node (leaf or subtree) within a hash tree.
///
/// The proof covers chunks `[start_chunk..end_chunk)`. For a single leaf,
/// `end_chunk == start_chunk + 1`. For an internal node, the range covers
/// all leaves under that subtree.
///
/// Siblings are stored root-to-leaf (outermost first). Fixed-size buffer
/// supports trees up to 2^64 chunks (depth ≤ 64).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InclusionProof {
    pub start_chunk: u64,
    pub end_chunk: u64,
    pub num_chunks: u64,
    buf: [Sibling; MAX_TREE_DEPTH],
    depth: usize,
}

impl InclusionProof {
    /// The siblings in this proof (root-to-leaf order).
    pub fn siblings(&self) -> &[Sibling] {
        &self.buf[..self.depth]
    }

    /// The proof depth (number of siblings).
    pub fn depth(&self) -> usize {
        self.depth
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for InclusionProof {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut s = serializer.serialize_struct("InclusionProof", 4)?;
        s.serialize_field("start_chunk", &self.start_chunk)?;
        s.serialize_field("end_chunk", &self.end_chunk)?;
        s.serialize_field("num_chunks", &self.num_chunks)?;
        s.serialize_field("siblings", self.siblings())?;
        s.end()
    }
}

#[cfg(feature = "serde")]
#[allow(unknown_lints, rs_no_vec)]
impl<'de> serde::Deserialize<'de> for InclusionProof {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use alloc::vec::Vec as AllocVec;
        use serde::de::{self, MapAccess, SeqAccess};

        #[derive(serde::Deserialize)]
        #[serde(field_identifier, rename_all = "snake_case")]
        enum Field { StartChunk, EndChunk, NumChunks, Siblings }

        struct InclusionProofVisitor;

        impl<'de> de::Visitor<'de> for InclusionProofVisitor {
            type Value = InclusionProof;

            fn expecting(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
                f.write_str("struct InclusionProof")
            }

            fn visit_seq<V: SeqAccess<'de>>(self, mut seq: V) -> Result<InclusionProof, V::Error> {
                let start_chunk = seq.next_element()?.ok_or_else(|| de::Error::invalid_length(0, &self))?;
                let end_chunk = seq.next_element()?.ok_or_else(|| de::Error::invalid_length(1, &self))?;
                let num_chunks = seq.next_element()?.ok_or_else(|| de::Error::invalid_length(2, &self))?;
                let siblings: AllocVec<Sibling> = seq.next_element()?.ok_or_else(|| de::Error::invalid_length(3, &self))?;
                if siblings.len() > MAX_TREE_DEPTH {
                    return Err(de::Error::custom("too many siblings"));
                }
                let mut buf = [SIBLING_ZERO; MAX_TREE_DEPTH];
                buf[..siblings.len()].copy_from_slice(&siblings);
                Ok(InclusionProof { start_chunk, end_chunk, num_chunks, buf, depth: siblings.len() })
            }

            fn visit_map<V: MapAccess<'de>>(self, mut map: V) -> Result<InclusionProof, V::Error> {
                let mut start_chunk = None;
                let mut end_chunk = None;
                let mut num_chunks = None;
                let mut siblings: Option<AllocVec<Sibling>> = None;
                while let Some(key) = map.next_key()? {
                    match key {
                        Field::StartChunk => { start_chunk = Some(map.next_value()?); }
                        Field::EndChunk => { end_chunk = Some(map.next_value()?); }
                        Field::NumChunks => { num_chunks = Some(map.next_value()?); }
                        Field::Siblings => { siblings = Some(map.next_value()?); }
                    }
                }
                let start_chunk = start_chunk.ok_or_else(|| de::Error::missing_field("start_chunk"))?;
                let end_chunk = end_chunk.ok_or_else(|| de::Error::missing_field("end_chunk"))?;
                let num_chunks = num_chunks.ok_or_else(|| de::Error::missing_field("num_chunks"))?;
                let siblings = siblings.ok_or_else(|| de::Error::missing_field("siblings"))?;
                if siblings.len() > MAX_TREE_DEPTH {
                    return Err(de::Error::custom("too many siblings"));
                }
                let mut buf = [SIBLING_ZERO; MAX_TREE_DEPTH];
                buf[..siblings.len()].copy_from_slice(&siblings);
                Ok(InclusionProof { start_chunk, end_chunk, num_chunks, buf, depth: siblings.len() })
            }
        }

        const FIELDS: &[&str] = &["start_chunk", "end_chunk", "num_chunks", "siblings"];
        deserializer.deserialize_struct("InclusionProof", FIELDS, InclusionProofVisitor)
    }
}

/// Sentinel value for unoccupied proof slots.
const SIBLING_ZERO: Sibling = Sibling::Left(Hash::from_bytes([0u8; params::OUTPUT_BYTES]));

/// Generate an inclusion proof for a single chunk.
///
/// Convenience wrapper around [`prove_range`] for `chunk_index..chunk_index+1`.
pub fn prove(data: &[u8], chunk_index: u64) -> (Hash, InclusionProof) {
    prove_range(data, chunk_index, chunk_index + 1)
}

/// Generate an inclusion proof for a range of chunks `[start..end)`.
///
/// The range must align to a subtree boundary in the left-balanced tree.
/// Returns the root hash and the proof. Panics if the range is invalid.
/// No heap allocation — uses a fixed-size siblings buffer.
pub fn prove_range(data: &[u8], start: u64, end: u64) -> (Hash, InclusionProof) {
    let n = if data.is_empty() { 1u64 } else { ((data.len() + CHUNK_SIZE - 1) / CHUNK_SIZE) as u64 };
    assert!(start < end && end <= n, "invalid range [{start}..{end}) for {n} chunks");

    if n == 1 {
        let cv = hash_leaf(data, 0, true);
        return (
            cv,
            InclusionProof {
                start_chunk: start,
                end_chunk: end,
                num_chunks: n,
                buf: [SIBLING_ZERO; MAX_TREE_DEPTH],
                depth: 0,
            },
        );
    }

    let mut buf = [SIBLING_ZERO; MAX_TREE_DEPTH];
    let mut depth = 0usize;
    let root = prove_subtree_data(
        data,
        0,
        n as usize,
        start as usize,
        end as usize,
        true,
        &mut buf,
        &mut depth,
    );
    (
        root,
        InclusionProof {
            start_chunk: start,
            end_chunk: end,
            num_chunks: n,
            buf,
            depth,
        },
    )
}

/// Walk the left-balanced tree over data, collecting siblings along the
/// path to `[target_start..target_end)`. Computes leaf hashes on demand.
fn prove_subtree_data(
    data: &[u8],
    offset: usize,
    count: usize,
    target_start: usize,
    target_end: usize,
    is_root: bool,
    buf: &mut [Sibling; MAX_TREE_DEPTH],
    depth: &mut usize,
) -> Hash {
    debug_assert!(count > 0);

    // If the current subtree exactly matches the target range, just merge it.
    if target_start == offset && target_end == offset + count {
        return merge_range(data, offset, count, is_root);
    }

    assert!(count > 1, "target range does not align to a subtree boundary");

    let split = 1 << (usize::BITS - (count - 1).leading_zeros() - 1);

    if target_end <= offset + split {
        // Target is entirely in the left subtree.
        let right = merge_range(data, offset + split, count - split, false);
        buf[*depth] = Sibling::Right(right);
        *depth += 1;
        let left = prove_subtree_data(
            data, offset, split, target_start, target_end, false, buf, depth,
        );
        hash_node(&left, &right, is_root)
    } else if target_start >= offset + split {
        // Target is entirely in the right subtree.
        let left = merge_range(data, offset, split, false);
        buf[*depth] = Sibling::Left(left);
        *depth += 1;
        let right = prove_subtree_data(
            data, offset + split, count - split, target_start, target_end, false, buf, depth,
        );
        hash_node(&left, &right, is_root)
    } else {
        panic!(
            "range [{target_start}..{target_end}) straddles split at {} — \
             not a valid subtree boundary",
            offset + split,
        );
    }
}

/// Verify an inclusion proof for a leaf chunk against an expected root hash.
///
/// Given the raw chunk data, recomputes the leaf hash and walks the
/// proof path up to the root. Returns `true` if the recomputed root
/// matches `expected_root`.
pub fn verify_proof(
    chunk_data: &[u8],
    proof: &InclusionProof,
    expected_root: &Hash,
) -> bool {
    assert_eq!(
        proof.end_chunk - proof.start_chunk,
        1,
        "verify_proof is for single chunks; use verify_node_proof for subtrees"
    );
    let is_single = proof.num_chunks == 1;
    let mut current = hash_leaf(chunk_data, proof.start_chunk, is_single);

    if is_single {
        return current == *expected_root;
    }

    walk_proof(&mut current, proof.siblings())
        == *expected_root
}

/// Verify an inclusion proof for an internal node (subtree hash) against
/// an expected root hash.
pub fn verify_node_proof(
    node_hash: &Hash,
    proof: &InclusionProof,
    expected_root: &Hash,
) -> bool {
    if proof.siblings().is_empty() {
        return *node_hash == *expected_root;
    }

    let mut current = *node_hash;
    walk_proof(&mut current, proof.siblings())
        == *expected_root
}

/// Walk siblings from leaf toward root, returning the computed root.
fn walk_proof(current: &mut Hash, siblings: &[Sibling]) -> Hash {
    let depth = siblings.len();
    for (i, sibling) in siblings.iter().rev().enumerate() {
        let is_root = i == depth - 1;
        *current = match sibling {
            Sibling::Left(left) => hash_node(left, current, is_root),
            Sibling::Right(right) => hash_node(current, right, is_root),
        };
    }
    *current
}

// ── In-order tree indexing ───────────────────────────────────────

/// In-order index for addressing any node in the tree.
///
/// Leaves are at even indices (0, 2, 4, ...), parents at odd indices.
/// The level of a node is `trailing_ones()` of its index:
/// - Level 0 (leaves): indices 0, 2, 4, 6, ...
/// - Level 1: indices 1, 5, 9, 13, ...
/// - Level 2: indices 3, 11, 19, ...
/// - Level k: index has k trailing 1-bits
///
/// Navigation is pure arithmetic on the index.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NodeIndex(pub u64);

impl NodeIndex {
    /// The level of this node (0 = leaf).
    pub fn level(self) -> u32 {
        self.0.trailing_ones()
    }

    /// Whether this is a leaf node.
    pub fn is_leaf(self) -> bool {
        (self.0 & 1) == 0
    }

    /// The left child of this node, if it's not a leaf.
    pub fn left_child(self) -> Option<Self> {
        let level = self.level();
        if level == 0 {
            return None;
        }
        Some(Self(self.0 - (1 << (level - 1))))
    }

    /// The right child of this node, if it's not a leaf.
    pub fn right_child(self) -> Option<Self> {
        let level = self.level();
        if level == 0 {
            return None;
        }
        Some(Self(self.0 + (1 << (level - 1))))
    }

    /// The parent of this node.
    pub fn parent(self) -> Self {
        let level = self.level();
        let span = 1u64 << level;
        if (self.0 >> (level + 1)) & 1 == 0 {
            // We are a left child; parent is to the right.
            Self(self.0 + span)
        } else {
            // We are a right child; parent is to the left.
            Self(self.0 - span)
        }
    }

    /// The sibling of this node (the other child of our parent).
    pub fn sibling(self) -> Self {
        let level = self.level();
        let span = 1u64 << (level + 1);
        if (self.0 >> (level + 1)) & 1 == 0 {
            // We are a left child; sibling is to our right.
            Self(self.0 + span)
        } else {
            // We are a right child; sibling is to our left.
            Self(self.0 - span)
        }
    }

    /// Convert a chunk (leaf) index to an in-order index.
    pub fn from_chunk(chunk_index: u64) -> Self {
        Self(chunk_index * 2)
    }

    /// Convert an in-order leaf index back to a chunk index.
    /// Returns `None` if this is not a leaf.
    pub fn to_chunk(self) -> Option<u64> {
        if self.is_leaf() {
            Some(self.0 / 2)
        } else {
            None
        }
    }

    /// The root index for a tree with `n` leaves.
    ///
    /// In a left-balanced tree, the left subtree always has
    /// `split = 2^(ceil(log2(n)) - 1)` leaves, so the root's
    /// in-order index is always `split * 2 - 1`.
    pub fn root(n: u64) -> Self {
        if n <= 1 {
            return Self(0);
        }
        let split = 1u64 << (u64::BITS - (n - 1).leading_zeros() - 1);
        Self(split * 2 - 1)
    }
}

impl core::fmt::Display for NodeIndex {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Return the number of chunks for a given data length.
pub fn num_chunks(data_len: usize) -> u64 {
    if data_len == 0 {
        1
    } else {
        ((data_len + CHUNK_SIZE - 1) / CHUNK_SIZE) as u64
    }
}

/// Return the tree depth (number of levels above the leaves) for `n` chunks.
pub fn tree_depth(n: u64) -> u32 {
    if n <= 1 {
        0
    } else {
        u64::BITS - (n - 1).leading_zeros()
    }
}

/// A node in the hash tree with its position and children.
#[derive(Clone, Debug)]
#[allow(unknown_lints, rs_no_box)]
pub struct TreeNode {
    pub hash: Hash,
    pub depth: u32,
    pub index: NodeIndex,
    pub left: Option<alloc::boxed::Box<TreeNode>>,
    pub right: Option<alloc::boxed::Box<TreeNode>>,
    /// For leaves: the chunk index.
    pub chunk_index: Option<u64>,
}

/// Build the full hash tree and return the root node.
///
/// Each node contains its hash, depth, in-order index, and references
/// to children. Leaves have `chunk_index` set and no children.
///
/// Uses heap allocation: the tree is data-proportional and built for
/// display/inspection, not for the hot hashing path.
#[allow(unknown_lints, rs_no_vec, rs_no_box)]
pub fn build_tree(data: &[u8]) -> TreeNode {
    let chunks: alloc::vec::Vec<&[u8]> = if data.is_empty() {
        alloc::vec![data]
    } else {
        data.chunks(CHUNK_SIZE).collect()
    };

    let leaves: alloc::vec::Vec<TreeNode> = chunks
        .iter()
        .enumerate()
        .map(|(i, chunk)| TreeNode {
            hash: hash_leaf(chunk, i as u64, chunks.len() == 1),
            depth: 0,
            index: NodeIndex::from_chunk(i as u64),
            left: None,
            right: None,
            chunk_index: Some(i as u64),
        })
        .collect();

    if leaves.len() == 1 {
        return leaves.into_iter().next().unwrap();
    }

    build_subtree(leaves, 0, true)
}

/// Build full tree with progress callback.
///
/// `progress` receives `(completed, total)` where total = 2n−1.
#[allow(unknown_lints, rs_no_vec, rs_no_box)]
pub fn build_tree_with_progress(data: &[u8], progress: impl Fn(usize, usize)) -> TreeNode {
    let chunks: alloc::vec::Vec<&[u8]> = if data.is_empty() {
        alloc::vec![data]
    } else {
        data.chunks(CHUNK_SIZE).collect()
    };

    let n = chunks.len();
    let total = if n <= 1 { 1 } else { 2 * n - 1 };
    let done = core::cell::Cell::new(0usize);

    let leaves: alloc::vec::Vec<TreeNode> = chunks
        .iter()
        .enumerate()
        .map(|(i, chunk)| {
            let node = TreeNode {
                hash: hash_leaf(chunk, i as u64, n == 1),
                depth: 0,
                index: NodeIndex::from_chunk(i as u64),
                left: None,
                right: None,
                chunk_index: Some(i as u64),
            };
            let v = done.get() + 1;
            done.set(v);
            progress(v, total);
            node
        })
        .collect();

    if leaves.len() == 1 {
        return leaves.into_iter().next().unwrap();
    }

    build_subtree_progress(leaves, 0, true, &done, total, &progress)
}

/// Recursive subtree builder with progress.
#[allow(unknown_lints, rs_no_vec, rs_no_box)]
fn build_subtree_progress(
    nodes: alloc::vec::Vec<TreeNode>,
    base_offset: u64,
    is_root: bool,
    done: &core::cell::Cell<usize>,
    total: usize,
    progress: &impl Fn(usize, usize),
) -> TreeNode {
    if nodes.len() == 1 {
        return nodes.into_iter().next().unwrap();
    }

    let n = nodes.len();
    let split = 1 << (usize::BITS - (n - 1).leading_zeros() - 1);
    let (left_nodes, right_nodes): (alloc::vec::Vec<_>, alloc::vec::Vec<_>) = {
        let mut iter = nodes.into_iter();
        let left: alloc::vec::Vec<_> = iter.by_ref().take(split).collect();
        let right: alloc::vec::Vec<_> = iter.collect();
        (left, right)
    };

    let left = build_subtree_progress(left_nodes, base_offset, false, done, total, progress);
    let right_offset = base_offset + (split as u64) * 2;
    let right = build_subtree_progress(right_nodes, right_offset, false, done, total, progress);
    let hash = hash_node(&left.hash, &right.hash, is_root);
    let depth = left.depth.max(right.depth) + 1;
    let root_index = base_offset + (split as u64) * 2 - 1;

    let v = done.get() + 1;
    done.set(v);
    progress(v, total);

    TreeNode {
        hash,
        depth,
        index: NodeIndex(root_index),
        left: Some(alloc::boxed::Box::new(left)),
        right: Some(alloc::boxed::Box::new(right)),
        chunk_index: None,
    }
}

/// `base_offset` is the in-order index offset for the first leaf in this subtree.
#[allow(unknown_lints, rs_no_vec, rs_no_box)]
fn build_subtree(nodes: alloc::vec::Vec<TreeNode>, base_offset: u64, is_root: bool) -> TreeNode {
    if nodes.len() == 1 {
        return nodes.into_iter().next().unwrap();
    }

    let n = nodes.len();
    let split = 1 << (usize::BITS - (n - 1).leading_zeros() - 1);
    let (left_nodes, right_nodes): (alloc::vec::Vec<_>, alloc::vec::Vec<_>) = {
        let mut iter = nodes.into_iter();
        let left: alloc::vec::Vec<_> = iter.by_ref().take(split).collect();
        let right: alloc::vec::Vec<_> = iter.collect();
        (left, right)
    };

    let left = build_subtree(left_nodes, base_offset, false);
    // Right subtree starts after left subtree + root node in in-order layout.
    let right_offset = base_offset + (split as u64) * 2;
    let right = build_subtree(right_nodes, right_offset, false);
    let hash = hash_node(&left.hash, &right.hash, is_root);
    let depth = left.depth.max(right.depth) + 1;

    // Root of this subtree: in-order index is between left and right subtrees.
    let root_index = base_offset + (split as u64) * 2 - 1;

    TreeNode {
        hash,
        depth,
        index: NodeIndex(root_index),
        left: Some(alloc::boxed::Box::new(left)),
        right: Some(alloc::boxed::Box::new(right)),
        chunk_index: None,
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::vec;
    use super::*;
    use crate::params::OUTPUT_BYTES;

    #[test]
    fn hash_node_non_commutative() {
        let left = Hash::from_bytes([1u8; OUTPUT_BYTES]);
        let right = Hash::from_bytes([2u8; OUTPUT_BYTES]);
        let lr = hash_node(&left, &right, false);
        let rl = hash_node(&right, &left, false);
        assert_ne!(lr, rl);
    }

    #[test]
    fn hash_node_root_differs() {
        let left = Hash::from_bytes([1u8; OUTPUT_BYTES]);
        let right = Hash::from_bytes([2u8; OUTPUT_BYTES]);
        let non_root = hash_node(&left, &right, false);
        let root = hash_node(&left, &right, true);
        assert_ne!(non_root, root);
    }

    #[test]
    fn hash_leaf_root_differs() {
        let data = b"chunk data";
        let non_root = hash_leaf(data, 0, false);
        let root = hash_leaf(data, 0, true);
        assert_ne!(non_root, root);
    }

    #[test]
    fn hash_leaf_counter_differs() {
        let data = b"chunk data";
        let c0 = hash_leaf(data, 0, false);
        let c1 = hash_leaf(data, 1, false);
        assert_ne!(c0, c1);
    }

    #[test]
    fn hash_node_deterministic() {
        let left = Hash::from_bytes([0xAA; OUTPUT_BYTES]);
        let right = Hash::from_bytes([0xBB; OUTPUT_BYTES]);
        let h1 = hash_node(&left, &right, false);
        let h2 = hash_node(&left, &right, false);
        assert_eq!(h1, h2);
    }

    #[test]
    fn hash_leaf_different_data() {
        let h1 = hash_leaf(b"data1", 0, false);
        let h2 = hash_leaf(b"data2", 0, false);
        assert_ne!(h1, h2);
    }

    #[test]
    fn hash_leaf_empty() {
        let h = hash_leaf(b"", 0, false);
        assert_ne!(h.as_bytes(), &[0u8; OUTPUT_BYTES]);
    }

    #[test]
    fn hash_leaf_vs_plain_hash() {
        // hash_leaf should differ from a plain hash of the same data
        // because of the CHUNK flag domain separation.
        let data = b"test data";
        let plain = crate::sponge::Hasher::new().update(data).finalize();
        let cv = hash_leaf(data, 0, false);
        assert_ne!(plain, cv);
    }

    // ── Multi-level tree tests ─────────────────────────────────────

    #[test]
    fn two_chunk_tree() {
        // Simulate a 2-chunk file: left + right → root parent
        let left_cv = hash_leaf(b"chunk 0 data", 0, false);
        let right_cv = hash_leaf(b"chunk 1 data", 1, false);
        let root = hash_node(&left_cv, &right_cv, true);
        assert_ne!(root.as_bytes(), &[0u8; OUTPUT_BYTES]);

        // Deterministic
        let root2 = hash_node(&left_cv, &right_cv, true);
        assert_eq!(root, root2);
    }

    #[test]
    fn four_chunk_tree() {
        // 4-chunk balanced tree:
        //        root
        //       /    \
        //     p01    p23
        //    / \    / \
        //  c0  c1 c2  c3
        let c0 = hash_leaf(b"chunk0", 0, false);
        let c1 = hash_leaf(b"chunk1", 1, false);
        let c2 = hash_leaf(b"chunk2", 2, false);
        let c3 = hash_leaf(b"chunk3", 3, false);

        let p01 = hash_node(&c0, &c1, false);
        let p23 = hash_node(&c2, &c3, false);
        let root = hash_node(&p01, &p23, true);

        assert_ne!(root.as_bytes(), &[0u8; OUTPUT_BYTES]);

        // Permuting children changes the root
        let p01_swapped = hash_node(&c1, &c0, false);
        assert_ne!(p01, p01_swapped);

        let root_swapped = hash_node(&p01_swapped, &p23, true);
        assert_ne!(root, root_swapped);
    }

    #[test]
    fn single_chunk_root_vs_multi_chunk() {
        // A single-chunk file has is_root=true
        // A multi-chunk file's first chunk has is_root=false
        let data = b"single chunk";
        let single_root = hash_leaf(data, 0, true);
        let multi_first = hash_leaf(data, 0, false);
        assert_ne!(single_root, multi_first);
    }

    #[test]
    fn hash_node_identical_children() {
        // Even with identical children, hash_node should produce a non-trivial hash
        let cv = hash_leaf(b"duplicate", 0, false);
        let parent = hash_node(&cv, &cv, false);
        assert_ne!(parent.as_bytes(), &[0u8; OUTPUT_BYTES]);
        // Parent should differ from either child
        assert_ne!(parent, cv);
    }

    #[test]
    fn hash_leaf_large_counter() {
        // Large counter values should work and produce different results
        let cv_max = hash_leaf(b"data", u64::MAX, false);
        let cv_zero = hash_leaf(b"data", 0, false);
        assert_ne!(cv_max, cv_zero);
    }

    #[test]
    fn hash_leaf_large_data() {
        // Chunk with > 1 rate block of data
        let data = vec![0xAB; 1000];
        let cv = hash_leaf(&data, 0, false);
        assert_ne!(cv.as_bytes(), &[0u8; OUTPUT_BYTES]);

        // Deterministic
        let cv2 = hash_leaf(&data, 0, false);
        assert_eq!(cv, cv2);
    }

    #[test]
    fn hash_node_non_root_vs_root_with_identical_children() {
        let cv = Hash::from_bytes([0x42; OUTPUT_BYTES]);
        let non_root = hash_node(&cv, &cv, false);
        let root = hash_node(&cv, &cv, true);
        assert_ne!(non_root, root);
    }

    // ── Flag isolation tests ───────────────────────────────────────

    #[test]
    fn flags_use_correct_capacity_indices() {
        // Verify that tree uses state[8] and state[9] (no overlap with sponge's state[10], state[11])
        assert_eq!(CAPACITY_COUNTER_IDX, 8);
        assert_eq!(CAPACITY_FLAGS_IDX, 9);
        // Sponge uses CAPACITY_START + 2 = 10 and CAPACITY_START + 3 = 11
        assert!(CAPACITY_COUNTER_IDX < 10);
        assert!(CAPACITY_FLAGS_IDX < 10);
    }

    #[test]
    fn flag_bits_are_distinct() {
        assert_eq!(FLAG_ROOT & FLAG_PARENT, 0);
        assert_eq!(FLAG_ROOT & FLAG_CHUNK, 0);
        assert_eq!(FLAG_PARENT & FLAG_CHUNK, 0);
    }

    // ── NMT parent tests ─────────────────────────────────────────

    #[test]
    fn hash_node_nmt_zero_ns_matches_hash_node() {
        let left = Hash::from_bytes([1u8; OUTPUT_BYTES]);
        let right = Hash::from_bytes([2u8; OUTPUT_BYTES]);
        let standard = hash_node(&left, &right, false);
        let nmt_zero = hash_node_nmt(&left, &right, 0, 0, false);
        assert_eq!(standard, nmt_zero);
    }

    #[test]
    fn hash_node_nmt_ns_differs_from_plain() {
        let left = Hash::from_bytes([1u8; OUTPUT_BYTES]);
        let right = Hash::from_bytes([2u8; OUTPUT_BYTES]);
        let plain = hash_node(&left, &right, false);
        let with_ns = hash_node_nmt(&left, &right, 10, 20, false);
        assert_ne!(plain, with_ns);
    }

    #[test]
    fn hash_node_nmt_different_ns_ranges() {
        let left = Hash::from_bytes([1u8; OUTPUT_BYTES]);
        let right = Hash::from_bytes([2u8; OUTPUT_BYTES]);
        let h1 = hash_node_nmt(&left, &right, 10, 20, false);
        let h2 = hash_node_nmt(&left, &right, 10, 30, false);
        assert_ne!(h1, h2);
    }

    #[test]
    fn hash_node_nmt_root_differs() {
        let left = Hash::from_bytes([1u8; OUTPUT_BYTES]);
        let right = Hash::from_bytes([2u8; OUTPUT_BYTES]);
        let non_root = hash_node_nmt(&left, &right, 5, 15, false);
        let root = hash_node_nmt(&left, &right, 5, 15, true);
        assert_ne!(non_root, root);
    }

    #[test]
    fn hash_node_nmt_non_commutative() {
        let left = Hash::from_bytes([1u8; OUTPUT_BYTES]);
        let right = Hash::from_bytes([2u8; OUTPUT_BYTES]);
        let lr = hash_node_nmt(&left, &right, 5, 15, false);
        let rl = hash_node_nmt(&right, &left, 5, 15, false);
        assert_ne!(lr, rl);
    }

    #[test]
    fn nmt_capacity_indices_no_overlap() {
        assert_eq!(CAPACITY_NS_MIN_IDX, 12);
        assert_eq!(CAPACITY_NS_MAX_IDX, 13);
        // No overlap with counter (8), flags (9), msg_length (10), domain (11)
        assert!(CAPACITY_NS_MIN_IDX > 11);
    }

    // ── root_hash tests ──────────────────────────────────────────

    #[test]
    fn root_hash_empty() {
        let h = root_hash(b"");
        assert_eq!(h, hash_leaf(b"", 0, true));
    }

    #[test]
    fn root_hash_single_chunk() {
        let data = vec![0x42u8; 100];
        let h = root_hash(&data);
        assert_eq!(h, hash_leaf(&data, 0, true));
    }

    #[test]
    fn root_hash_exact_chunk() {
        let data = vec![0x42u8; CHUNK_SIZE];
        let h = root_hash(&data);
        assert_eq!(h, hash_leaf(&data, 0, true));
    }

    #[test]
    fn root_hash_two_chunks() {
        let data = vec![0x42u8; CHUNK_SIZE + 1];
        let h = root_hash(&data);
        let c0 = hash_leaf(&data[..CHUNK_SIZE], 0, false);
        let c1 = hash_leaf(&data[CHUNK_SIZE..], 1, false);
        assert_eq!(h, hash_node(&c0, &c1, true));
    }

    #[test]
    fn root_hash_three_chunks() {
        // 3 chunks → left-balanced: left subtree has 2 leaves, right has 1
        let data = vec![0xAB; CHUNK_SIZE * 3];
        let h = root_hash(&data);

        let c0 = hash_leaf(&data[..CHUNK_SIZE], 0, false);
        let c1 = hash_leaf(&data[CHUNK_SIZE..CHUNK_SIZE * 2], 1, false);
        let c2 = hash_leaf(&data[CHUNK_SIZE * 2..], 2, false);
        let left = hash_node(&c0, &c1, false);
        let expected = hash_node(&left, &c2, true);
        assert_eq!(h, expected);
    }

    #[test]
    fn root_hash_four_chunks() {
        let data = vec![0xCD; CHUNK_SIZE * 4];
        let h = root_hash(&data);

        let c0 = hash_leaf(&data[..CHUNK_SIZE], 0, false);
        let c1 = hash_leaf(&data[CHUNK_SIZE..CHUNK_SIZE * 2], 1, false);
        let c2 = hash_leaf(&data[CHUNK_SIZE * 2..CHUNK_SIZE * 3], 2, false);
        let c3 = hash_leaf(&data[CHUNK_SIZE * 3..], 3, false);
        let p01 = hash_node(&c0, &c1, false);
        let p23 = hash_node(&c2, &c3, false);
        let expected = hash_node(&p01, &p23, true);
        assert_eq!(h, expected);
    }

    #[test]
    fn root_hash_deterministic() {
        let data = vec![0xEF; CHUNK_SIZE * 5];
        assert_eq!(root_hash(&data), root_hash(&data));
    }

    #[test]
    fn root_hash_differs_from_plain_hash() {
        // root_hash uses hash_leaf with flags; plain hash does not
        let data = b"small input";
        let th = root_hash(data);
        let ph = crate::hash(data);
        assert_ne!(th, ph);
    }

    // ── Inclusion proof tests ─────────────────────────────────────

    #[test]
    fn prove_single_chunk() {
        let data = b"small data";
        let (root, proof) = prove(data, 0);
        assert_eq!(root, root_hash(data));
        assert_eq!(proof.depth(), 0);
        assert!(verify_proof(data, &proof, &root));
    }

    #[test]
    fn prove_empty() {
        let data = b"";
        let (root, proof) = prove(data, 0);
        assert_eq!(root, root_hash(data));
        assert!(verify_proof(data, &proof, &root));
    }

    #[test]
    fn prove_two_chunks() {
        let data = vec![0x42u8; CHUNK_SIZE + 1];
        let root = root_hash(&data);

        // Prove chunk 0
        let (r0, p0) = prove(&data, 0);
        assert_eq!(r0, root);
        assert_eq!(p0.depth(), 1);
        assert!(verify_proof(&data[..CHUNK_SIZE], &p0, &root));

        // Prove chunk 1
        let (r1, p1) = prove(&data, 1);
        assert_eq!(r1, root);
        assert_eq!(p1.depth(), 1);
        assert!(verify_proof(&data[CHUNK_SIZE..], &p1, &root));
    }

    #[test]
    fn prove_four_chunks() {
        let data = vec![0xCD; CHUNK_SIZE * 4];
        let root = root_hash(&data);

        for i in 0..4u64 {
            let start = i as usize * CHUNK_SIZE;
            let end = start + CHUNK_SIZE;
            let (r, proof) = prove(&data, i);
            assert_eq!(r, root);
            assert_eq!(proof.depth(), 2); // depth = 2
            assert!(verify_proof(&data[start..end], &proof, &root));
        }
    }

    #[test]
    fn prove_three_chunks() {
        // Left-balanced: left subtree has 2 leaves, right has 1
        let data = vec![0xAB; CHUNK_SIZE * 3];
        let root = root_hash(&data);

        for i in 0..3u64 {
            let start = i as usize * CHUNK_SIZE;
            let end = start + CHUNK_SIZE;
            let (r, proof) = prove(&data, i);
            assert_eq!(r, root);
            assert!(verify_proof(&data[start..end], &proof, &root));
        }

        // Chunks 0,1 have depth 2; chunk 2 has depth 1
        let (_, p0) = prove(&data, 0);
        let (_, p2) = prove(&data, 2);
        assert_eq!(p0.depth(), 2);
        assert_eq!(p2.depth(), 1);
    }

    #[test]
    fn prove_five_chunks() {
        let data = vec![0xEF; CHUNK_SIZE * 5];
        let root = root_hash(&data);

        for i in 0..5u64 {
            let start = i as usize * CHUNK_SIZE;
            let end = start + CHUNK_SIZE;
            let (r, proof) = prove(&data, i);
            assert_eq!(r, root);
            assert!(verify_proof(&data[start..end], &proof, &root));
        }
    }

    #[test]
    fn verify_proof_wrong_data_fails() {
        let data = vec![0x42u8; CHUNK_SIZE * 2];
        let root = root_hash(&data);
        let (_, proof) = prove(&data, 0);

        let wrong_chunk = vec![0xFF; CHUNK_SIZE];
        assert!(!verify_proof(&wrong_chunk, &proof, &root));
    }

    #[test]
    fn verify_proof_wrong_root_fails() {
        let data = vec![0x42u8; CHUNK_SIZE * 2];
        let (_, proof) = prove(&data, 0);

        let wrong_root = Hash::from_bytes([0xFF; OUTPUT_BYTES]);
        assert!(!verify_proof(&data[..CHUNK_SIZE], &proof, &wrong_root));
    }

    #[test]
    fn verify_proof_swapped_chunk_fails() {
        // Proof for chunk 0 should not verify with chunk 1's data
        let data = vec![0x42u8; CHUNK_SIZE + 100]; // 2 chunks, different sizes
        let root = root_hash(&data);
        let (_, p0) = prove(&data, 0);
        assert!(!verify_proof(&data[CHUNK_SIZE..], &p0, &root));
    }

    #[test]
    fn prove_large_file() {
        // 256 chunks = 1 MB
        let data = vec![0x77; CHUNK_SIZE * 256];
        let root = root_hash(&data);

        // Verify a few chunks
        for &i in &[0, 1, 127, 128, 255] {
            let start = i * CHUNK_SIZE;
            let end = start + CHUNK_SIZE;
            let (r, proof) = prove(&data, i as u64);
            assert_eq!(r, root);
            assert!(verify_proof(&data[start..end], &proof, &root));
            assert_eq!(proof.depth(), 8); // log2(256) = 8
        }
    }

    // ── Navigation helpers ────────────────────────────────────────

    #[test]
    fn num_chunks_basic() {
        assert_eq!(num_chunks(0), 1);
        assert_eq!(num_chunks(1), 1);
        assert_eq!(num_chunks(CHUNK_SIZE), 1);
        assert_eq!(num_chunks(CHUNK_SIZE + 1), 2);
        assert_eq!(num_chunks(CHUNK_SIZE * 4), 4);
        assert_eq!(num_chunks(CHUNK_SIZE * 4 + 1), 5);
    }

    #[test]
    fn tree_depth_basic() {
        assert_eq!(tree_depth(1), 0);
        assert_eq!(tree_depth(2), 1);
        assert_eq!(tree_depth(3), 2);
        assert_eq!(tree_depth(4), 2);
        assert_eq!(tree_depth(5), 3);
        assert_eq!(tree_depth(256), 8);
    }

    #[test]
    #[should_panic(expected = "invalid range")]
    fn prove_out_of_range_panics() {
        let data = b"small";
        prove(data, 1);
    }

    // ── Range / node proof tests ─────────────────────────────────

    #[test]
    fn prove_range_full_tree() {
        // Proving the entire range should give empty siblings
        let data = vec![0x42u8; CHUNK_SIZE * 4];
        let root = root_hash(&data);
        let (r, proof) = prove_range(&data, 0, 4);
        assert_eq!(r, root);
        assert_eq!(proof.depth(), 0);
        assert!(verify_node_proof(&root, &proof, &root));
    }

    #[test]
    fn prove_range_left_subtree() {
        // 4 chunks: prove left subtree [0..2)
        let data = vec![0xCD; CHUNK_SIZE * 4];
        let root = root_hash(&data);
        let (r, proof) = prove_range(&data, 0, 2);
        assert_eq!(r, root);
        assert_eq!(proof.depth(), 1); // just the right subtree

        // The node hash should be hash_node(c0, c1, false)
        let c0 = hash_leaf(&data[..CHUNK_SIZE], 0, false);
        let c1 = hash_leaf(&data[CHUNK_SIZE..CHUNK_SIZE * 2], 1, false);
        let left_node = hash_node(&c0, &c1, false);
        assert!(verify_node_proof(&left_node, &proof, &root));
    }

    #[test]
    fn prove_range_right_subtree() {
        // 4 chunks: prove right subtree [2..4)
        let data = vec![0xCD; CHUNK_SIZE * 4];
        let root = root_hash(&data);
        let (r, proof) = prove_range(&data, 2, 4);
        assert_eq!(r, root);
        assert_eq!(proof.depth(), 1);

        let c2 = hash_leaf(&data[CHUNK_SIZE * 2..CHUNK_SIZE * 3], 2, false);
        let c3 = hash_leaf(&data[CHUNK_SIZE * 3..], 3, false);
        let right_node = hash_node(&c2, &c3, false);
        assert!(verify_node_proof(&right_node, &proof, &root));
    }

    #[test]
    fn prove_range_nested_subtree() {
        // 8 chunks: prove [0..2) which is 2 levels deep
        let data = vec![0xAB; CHUNK_SIZE * 8];
        let root = root_hash(&data);
        let (r, proof) = prove_range(&data, 0, 2);
        assert_eq!(r, root);
        assert_eq!(proof.depth(), 2);

        let c0 = hash_leaf(&data[..CHUNK_SIZE], 0, false);
        let c1 = hash_leaf(&data[CHUNK_SIZE..CHUNK_SIZE * 2], 1, false);
        let node_hash = hash_node(&c0, &c1, false);
        assert!(verify_node_proof(&node_hash, &proof, &root));
    }

    #[test]
    #[should_panic(expected = "straddles split")]
    fn prove_range_unaligned_panics() {
        // [1..3) doesn't align to any subtree in a 4-chunk tree
        let data = vec![0xCD; CHUNK_SIZE * 4];
        prove_range(&data, 1, 3);
    }

    #[test]
    fn verify_node_proof_wrong_hash_fails() {
        let data = vec![0xCD; CHUNK_SIZE * 4];
        let root = root_hash(&data);
        let (_, proof) = prove_range(&data, 0, 2);
        let wrong = Hash::from_bytes([0xFF; OUTPUT_BYTES]);
        assert!(!verify_node_proof(&wrong, &proof, &root));
    }

    // ── NodeIndex tests ──────────────────────────────────────────

    #[test]
    fn node_index_leaf() {
        assert!(NodeIndex(0).is_leaf());
        assert!(NodeIndex(2).is_leaf());
        assert!(!NodeIndex(1).is_leaf());
        assert!(!NodeIndex(3).is_leaf());
    }

    #[test]
    fn node_index_level() {
        assert_eq!(NodeIndex(0).level(), 0); // leaf
        assert_eq!(NodeIndex(2).level(), 0); // leaf
        assert_eq!(NodeIndex(1).level(), 1); // parent of 0,2
        assert_eq!(NodeIndex(3).level(), 2); // parent of 1,5
        assert_eq!(NodeIndex(7).level(), 3); // root of 8-leaf tree
    }

    #[test]
    fn node_index_children() {
        // Node 1 (level 1): children are 0 and 2
        assert_eq!(NodeIndex(1).left_child(), Some(NodeIndex(0)));
        assert_eq!(NodeIndex(1).right_child(), Some(NodeIndex(2)));

        // Node 3 (level 2): children are 1 and 5
        assert_eq!(NodeIndex(3).left_child(), Some(NodeIndex(1)));
        assert_eq!(NodeIndex(3).right_child(), Some(NodeIndex(5)));

        // Node 7 (level 3): children are 3 and 11
        assert_eq!(NodeIndex(7).left_child(), Some(NodeIndex(3)));
        assert_eq!(NodeIndex(7).right_child(), Some(NodeIndex(11)));

        // Leaf has no children
        assert_eq!(NodeIndex(0).left_child(), None);
        assert_eq!(NodeIndex(0).right_child(), None);
    }

    #[test]
    fn node_index_parent() {
        assert_eq!(NodeIndex(0).parent(), NodeIndex(1));
        assert_eq!(NodeIndex(2).parent(), NodeIndex(1));
        assert_eq!(NodeIndex(1).parent(), NodeIndex(3));
        assert_eq!(NodeIndex(5).parent(), NodeIndex(3));
        assert_eq!(NodeIndex(3).parent(), NodeIndex(7));
        assert_eq!(NodeIndex(11).parent(), NodeIndex(7));
    }

    #[test]
    fn node_index_sibling() {
        assert_eq!(NodeIndex(0).sibling(), NodeIndex(2));
        assert_eq!(NodeIndex(2).sibling(), NodeIndex(0));
        assert_eq!(NodeIndex(1).sibling(), NodeIndex(5));
        assert_eq!(NodeIndex(5).sibling(), NodeIndex(1));
    }

    #[test]
    fn node_index_from_chunk() {
        assert_eq!(NodeIndex::from_chunk(0), NodeIndex(0));
        assert_eq!(NodeIndex::from_chunk(1), NodeIndex(2));
        assert_eq!(NodeIndex::from_chunk(3), NodeIndex(6));
    }

    #[test]
    fn node_index_to_chunk() {
        assert_eq!(NodeIndex(0).to_chunk(), Some(0));
        assert_eq!(NodeIndex(2).to_chunk(), Some(1));
        assert_eq!(NodeIndex(1).to_chunk(), None); // parent, not leaf
    }

    #[test]
    fn node_index_root() {
        assert_eq!(NodeIndex::root(1), NodeIndex(0));
        assert_eq!(NodeIndex::root(2), NodeIndex(1));
        assert_eq!(NodeIndex::root(3), NodeIndex(3));
        assert_eq!(NodeIndex::root(4), NodeIndex(3));
        assert_eq!(NodeIndex::root(5), NodeIndex(7));
        assert_eq!(NodeIndex::root(8), NodeIndex(7));
        assert_eq!(NodeIndex::root(9), NodeIndex(15));
    }

    #[test]
    fn build_tree_indices_match() {
        // Verify that build_tree assigns correct in-order indices
        let data = vec![0x42u8; CHUNK_SIZE * 4];
        let tree = build_tree(&data);

        // Root of 4-leaf tree should be at index 3
        assert_eq!(tree.index, NodeIndex(3));

        let left = tree.left.as_ref().unwrap();
        let right = tree.right.as_ref().unwrap();
        assert_eq!(left.index, NodeIndex(1));
        assert_eq!(right.index, NodeIndex(5));

        // Leaves
        let ll = left.left.as_ref().unwrap();
        let lr = left.right.as_ref().unwrap();
        let rl = right.left.as_ref().unwrap();
        let rr = right.right.as_ref().unwrap();
        assert_eq!(ll.index, NodeIndex(0));
        assert_eq!(lr.index, NodeIndex(2));
        assert_eq!(rl.index, NodeIndex(4));
        assert_eq!(rr.index, NodeIndex(6));
    }
}
