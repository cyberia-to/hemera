//! Hash tree construction.
//!
//! `hash_leaf` hashes leaf data into a chaining value.
//! `hash_node` combines two child chaining values into a parent node.

use crate::encoding::{bytes_to_cv, hash_to_bytes};
use crate::field::Goldilocks;
use crate::params::{self, CHUNK_SIZE, OUTPUT_ELEMENTS, RATE, WIDTH};
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
pub fn root_hash(data: &[u8]) -> Hash {
    if data.is_empty() {
        return hash_leaf(data, 0, true);
    }

    let chunks: Vec<&[u8]> = data.chunks(CHUNK_SIZE).collect();
    let cvs: Vec<Hash> = chunks
        .iter()
        .enumerate()
        .map(|(i, chunk)| hash_leaf(chunk, i as u64, chunks.len() == 1))
        .collect();

    if cvs.len() == 1 {
        return cvs[0];
    }

    merge_subtree(&cvs, true)
}

/// Recursively merge chaining values into a left-balanced binary tree.
fn merge_subtree(cvs: &[Hash], is_root: bool) -> Hash {
    debug_assert!(!cvs.is_empty());
    if cvs.len() == 1 {
        return cvs[0];
    }

    // Left subtree is a complete binary tree: split = 2^(ceil(log2(N)) - 1)
    let split = 1 << (usize::BITS - (cvs.len() - 1).leading_zeros() - 1);
    let left = merge_subtree(&cvs[..split], false);
    let right = merge_subtree(&cvs[split..], false);
    hash_node(&left, &right, is_root)
}

#[cfg(test)]
mod tests {
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
}
