//! Sparse Merkle tree: authenticated key-value storage.
//!
//! A sparse tree has a fixed depth (matching key length in bits) and
//! 2^depth possible leaf positions. Most positions are empty. Only
//! non-empty subtrees are stored; empty subtrees use precomputed
//! sentinel hashes.
//!
//! Keys are 32 bytes (256 bits). The path from root to leaf follows
//! key bits MSB-first: bit 0 of byte 0 is the first branching decision.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use crate::sponge::Hash;
use crate::tree::{hash_leaf, hash_node};

/// Default depth for 256-bit keys.
pub const DEFAULT_DEPTH: u32 = 256;

/// Extract the i-th path bit from a key (MSB-first).
/// bit_index 0 = most significant bit of byte[0].
fn key_bit(key: &[u8; 32], bit_index: u32) -> bool {
    let byte_idx = (bit_index / 8) as usize;
    let bit_in_byte = 7 - (bit_index % 8);
    (key[byte_idx] >> bit_in_byte) & 1 == 1
}

/// Mask a key to its first `prefix_len` bits (MSB-first), zeroing the rest.
fn mask_key(key: &[u8; 32], prefix_len: u32) -> [u8; 32] {
    let mut masked = [0u8; 32];
    let full_bytes = (prefix_len / 8) as usize;
    if full_bytes > 0 {
        masked[..full_bytes].copy_from_slice(&key[..full_bytes]);
    }
    let remaining_bits = prefix_len % 8;
    if remaining_bits > 0 && full_bytes < 32 {
        let m = 0xFFu8 << (8 - remaining_bits);
        masked[full_bytes] = key[full_bytes] & m;
    }
    masked
}

/// Compute sentinel hashes for all levels of a sparse tree.
///
/// `EMPTY[0]` = hash of empty leaf. `EMPTY[d]` = hash_node(EMPTY[d-1], EMPTY[d-1]).
#[allow(unknown_lints, rs_no_vec)]
pub fn sentinel_table(depth: u32) -> Vec<Hash> {
    let mut table = Vec::with_capacity(depth as usize + 1);
    table.push(hash_leaf(b"", 0, false));
    for d in 1..=depth {
        let prev = table[(d - 1) as usize];
        table.push(hash_node(&prev, &prev, d == depth));
    }
    table
}

/// Hash a sparse leaf: sponge(key || value), then structural binding.
///
/// The key is included in the hashed data to commit it in the leaf hash,
/// providing defense in depth alongside the tree path binding.
fn sparse_leaf_hash(key: &[u8; 32], value: &[u8]) -> Hash {
    let mut hasher = crate::sponge::Hasher::new();
    hasher.update(key);
    hasher.update(value);
    let base_hash = hasher.finalize();

    // Structural binding with FLAG_CHUNK, counter=0 (position encoded in tree path).
    use crate::encoding::{bytes_to_cv, hash_to_bytes};
    use crate::field::Goldilocks;
    use crate::params::{self, OUTPUT_ELEMENTS, RATE, WIDTH};

    let base_elems = bytes_to_cv(base_hash.as_bytes());
    let mut state = [Goldilocks::new(0); WIDTH];
    state[..OUTPUT_ELEMENTS].copy_from_slice(&base_elems);
    state[RATE] = Goldilocks::new(0); // counter = 0
    state[RATE + 1] = Goldilocks::new(1 << 2); // FLAG_CHUNK
    params::permute(&mut state);

    let output: [Goldilocks; OUTPUT_ELEMENTS] = state[..OUTPUT_ELEMENTS].try_into().unwrap();
    Hash::from_bytes(hash_to_bytes(&output))
}

/// Authenticated key-value store backed by a sparse Merkle tree.
#[derive(Debug)]
#[allow(unknown_lints, rs_no_vec)]
pub struct SparseTree {
    depth: u32,
    root: Hash,
    /// Internal node hashes: (level, masked_key) → hash.
    /// Level 0 = leaf, level `depth` = root.
    nodes: BTreeMap<(u32, [u8; 32]), Hash>,
    /// Leaf values: key → value.
    leaves: BTreeMap<[u8; 32], Vec<u8>>,
    /// Precomputed sentinel hashes for empty subtrees.
    sentinels: Vec<Hash>,
}

impl SparseTree {
    /// Create an empty sparse tree with the given depth.
    pub fn new(depth: u32) -> Self {
        let sentinels = sentinel_table(depth);
        let root = sentinels[depth as usize];
        Self {
            depth,
            root,
            nodes: BTreeMap::new(),
            leaves: BTreeMap::new(),
            sentinels,
        }
    }

    /// Create an empty sparse tree with default depth (256).
    pub fn new_default() -> Self {
        Self::new(DEFAULT_DEPTH)
    }

    /// The current root hash.
    pub fn root(&self) -> Hash {
        self.root
    }

    /// The tree depth.
    pub fn depth(&self) -> u32 {
        self.depth
    }

    /// Look up a value by key.
    pub fn get(&self, key: &[u8; 32]) -> Option<&[u8]> {
        self.leaves.get(key).map(|v| v.as_slice())
    }

    /// Number of populated leaves.
    pub fn len(&self) -> usize {
        self.leaves.len()
    }

    /// Whether the tree is empty.
    pub fn is_empty(&self) -> bool {
        self.leaves.is_empty()
    }

    /// Get the sentinel hash for a given level.
    fn sentinel(&self, level: u32) -> Hash {
        self.sentinels[level as usize]
    }

    /// Get the sibling hash at a given level for a key path.
    fn sibling_hash(&self, key: &[u8; 32], level: u32) -> Hash {
        let mut sib_key = *key;
        // Flip the bit at position (depth - 1 - level) to get sibling path.
        let bit_pos = self.depth - 1 - level;
        let byte_idx = (bit_pos / 8) as usize;
        let bit_in_byte = 7 - (bit_pos % 8);
        sib_key[byte_idx] ^= 1 << bit_in_byte;
        let masked = mask_key(&sib_key, self.depth - level);
        self.nodes.get(&(level, masked)).copied().unwrap_or(self.sentinel(level))
    }

    /// Insert or update a key-value pair. Returns the new root.
    #[allow(unknown_lints, rs_no_vec)]
    pub fn insert(&mut self, key: &[u8; 32], value: &[u8]) -> Hash {
        let leaf_hash = sparse_leaf_hash(key, value);
        self.leaves.insert(*key, Vec::from(value));

        // Store leaf hash at level 0.
        let masked_leaf = mask_key(key, self.depth);
        self.nodes.insert((0, masked_leaf), leaf_hash);

        // Walk from leaf to root, recomputing parent hashes.
        let mut current = leaf_hash;
        for level in 0..self.depth {
            let bit_pos = self.depth - 1 - level;
            let is_right = key_bit(key, bit_pos);
            let sibling = self.sibling_hash(key, level);
            let is_root = level + 1 == self.depth;

            let parent = if is_right {
                hash_node(&sibling, &current, is_root)
            } else {
                hash_node(&current, &sibling, is_root)
            };

            let parent_prefix = mask_key(key, self.depth - level - 1);
            self.nodes.insert((level + 1, parent_prefix), parent);
            current = parent;
        }

        self.root = current;
        self.root
    }

    /// Delete a key. Returns the new root.
    #[allow(unknown_lints, rs_no_vec)]
    pub fn delete(&mut self, key: &[u8; 32]) -> Hash {
        if self.leaves.remove(key).is_none() {
            return self.root;
        }

        // Remove leaf node.
        let masked_leaf = mask_key(key, self.depth);
        self.nodes.remove(&(0, masked_leaf));

        // Walk from leaf to root, pruning empty subtrees.
        let mut current = self.sentinel(0);
        for level in 0..self.depth {
            let bit_pos = self.depth - 1 - level;
            let is_right = key_bit(key, bit_pos);
            let sibling = self.sibling_hash(key, level);
            let is_root = level + 1 == self.depth;

            let parent_prefix = mask_key(key, self.depth - level - 1);

            if current == self.sentinel(level) && sibling == self.sentinel(level) {
                // Both children are sentinels — parent is sentinel too. Prune.
                self.nodes.remove(&(level + 1, parent_prefix));
                current = self.sentinel(level + 1);
            } else {
                let parent = if is_right {
                    hash_node(&sibling, &current, is_root)
                } else {
                    hash_node(&current, &sibling, is_root)
                };
                self.nodes.insert((level + 1, parent_prefix), parent);
                current = parent;
            }
        }

        self.root = current;
        self.root
    }

    /// Generate a compressed inclusion/non-inclusion proof for a key.
    #[allow(unknown_lints, rs_no_vec)]
    pub fn prove(&self, key: &[u8; 32]) -> CompressedSparseProof {
        let mut bitmask = [0u8; 32];
        let mut siblings = Vec::new();

        for level in 0..self.depth {
            let sibling = self.sibling_hash(key, level);
            if sibling != self.sentinel(level) {
                // Set bit in bitmask (bit `level` = 1 means real sibling).
                let byte_idx = (level / 8) as usize;
                let bit_in_byte = level % 8;
                bitmask[byte_idx] |= 1 << bit_in_byte;
                siblings.push(sibling);
            }
        }

        CompressedSparseProof {
            key: *key,
            bitmask,
            siblings,
        }
    }

    /// Verify a compressed proof for inclusion (value = Some) or non-inclusion (value = None).
    pub fn verify(proof: &CompressedSparseProof, value: Option<&[u8]>, root: &Hash, depth: u32) -> bool {
        let sentinels = sentinel_table(depth);
        Self::verify_with_sentinels(proof, value, root, depth, &sentinels)
    }

    fn verify_with_sentinels(
        proof: &CompressedSparseProof,
        value: Option<&[u8]>,
        root: &Hash,
        depth: u32,
        sentinels: &[Hash],
    ) -> bool {
        let mut current = match value {
            Some(v) => sparse_leaf_hash(&proof.key, v),
            None => sentinels[0],
        };

        let mut sib_cursor = 0usize;

        for level in 0..depth {
            let byte_idx = (level / 8) as usize;
            let bit_in_byte = level % 8;
            let has_real_sibling = (proof.bitmask[byte_idx] >> bit_in_byte) & 1 == 1;

            let sibling = if has_real_sibling {
                if sib_cursor >= proof.siblings.len() {
                    return false;
                }
                let s = proof.siblings[sib_cursor];
                sib_cursor += 1;
                s
            } else {
                sentinels[level as usize]
            };

            let bit_pos = depth - 1 - level;
            let is_right = key_bit(&proof.key, bit_pos);
            let is_root_level = level + 1 == depth;

            current = if is_right {
                hash_node(&sibling, &current, is_root_level)
            } else {
                hash_node(&current, &sibling, is_root_level)
            };
        }

        current == *root && sib_cursor == proof.siblings.len()
    }
}

/// Compressed sparse Merkle proof.
///
/// Sentinel siblings are encoded as 0-bits in the bitmask instead of
/// being stored explicitly. Only non-sentinel siblings appear in the
/// `siblings` vector.
#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(unknown_lints, rs_no_vec)]
pub struct CompressedSparseProof {
    pub key: [u8; 32],
    /// 256-bit bitmask: bit i = 1 means level i has a real (non-sentinel) sibling.
    pub bitmask: [u8; 32],
    /// Non-sentinel sibling hashes, in level order (level 0 first).
    pub siblings: Vec<Hash>,
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;

    #[test]
    fn empty_tree_root_is_sentinel() {
        let tree = SparseTree::new(8);
        let sentinels = sentinel_table(8);
        assert_eq!(tree.root(), sentinels[8]);
    }

    #[test]
    fn insert_changes_root() {
        let mut tree = SparseTree::new(8);
        let empty_root = tree.root();
        let key = [0u8; 32];
        tree.insert(&key, b"hello");
        assert_ne!(tree.root(), empty_root);
    }

    #[test]
    fn insert_then_get() {
        let mut tree = SparseTree::new(8);
        let key = [1u8; 32];
        tree.insert(&key, b"value");
        assert_eq!(tree.get(&key), Some(b"value".as_slice()));
    }

    #[test]
    fn insert_two_keys() {
        let mut tree = SparseTree::new(8);
        let k1 = [0u8; 32];
        let k2 = [0xFF; 32];
        tree.insert(&k1, b"one");
        tree.insert(&k2, b"two");
        assert_eq!(tree.get(&k1), Some(b"one".as_slice()));
        assert_eq!(tree.get(&k2), Some(b"two".as_slice()));
        assert_eq!(tree.len(), 2);
    }

    #[test]
    fn delete_restores_empty_root() {
        let mut tree = SparseTree::new(8);
        let empty_root = tree.root();
        let key = [0u8; 32];
        tree.insert(&key, b"value");
        assert_ne!(tree.root(), empty_root);
        tree.delete(&key);
        assert_eq!(tree.root(), empty_root);
        assert!(tree.is_empty());
    }

    #[test]
    fn delete_nonexistent_is_noop() {
        let mut tree = SparseTree::new(8);
        let key = [0u8; 32];
        tree.insert(&key, b"value");
        let root_before = tree.root();
        let other = [1u8; 32];
        tree.delete(&other);
        assert_eq!(tree.root(), root_before);
    }

    #[test]
    fn update_value() {
        let mut tree = SparseTree::new(8);
        let key = [0u8; 32];
        tree.insert(&key, b"old");
        let root_old = tree.root();
        tree.insert(&key, b"new");
        assert_ne!(tree.root(), root_old);
        assert_eq!(tree.get(&key), Some(b"new".as_slice()));
    }

    #[test]
    fn prove_inclusion() {
        let mut tree = SparseTree::new(8);
        let key = [0u8; 32];
        tree.insert(&key, b"value");
        let proof = tree.prove(&key);
        assert!(SparseTree::verify(&proof, Some(b"value"), &tree.root(), 8));
    }

    #[test]
    fn prove_non_inclusion() {
        let mut tree = SparseTree::new(8);
        let k1 = [0u8; 32];
        tree.insert(&k1, b"exists");
        let absent = [0xFF; 32];
        let proof = tree.prove(&absent);
        assert!(SparseTree::verify(&proof, None, &tree.root(), 8));
    }

    #[test]
    fn prove_wrong_value_fails() {
        let mut tree = SparseTree::new(8);
        let key = [0u8; 32];
        tree.insert(&key, b"correct");
        let proof = tree.prove(&key);
        assert!(!SparseTree::verify(&proof, Some(b"wrong"), &tree.root(), 8));
    }

    #[test]
    fn prove_inclusion_claimed_absent_fails() {
        let mut tree = SparseTree::new(8);
        let key = [0u8; 32];
        tree.insert(&key, b"exists");
        let proof = tree.prove(&key);
        // Claiming key is absent should fail.
        assert!(!SparseTree::verify(&proof, None, &tree.root(), 8));
    }

    #[test]
    fn compressed_proof_smaller_than_full() {
        let mut tree = SparseTree::new(16);
        let key = [0u8; 32];
        tree.insert(&key, b"value");
        let proof = tree.prove(&key);
        // Most siblings are sentinels — compressed proof has far fewer than `depth` entries.
        assert!(proof.siblings.len() < 16);
    }

    #[test]
    fn two_keys_proofs_both_valid() {
        let mut tree = SparseTree::new(8);
        let k1 = [0u8; 32];
        let k2 = [0xFF; 32];
        tree.insert(&k1, b"one");
        tree.insert(&k2, b"two");

        let p1 = tree.prove(&k1);
        let p2 = tree.prove(&k2);
        assert!(SparseTree::verify(&p1, Some(b"one"), &tree.root(), 8));
        assert!(SparseTree::verify(&p2, Some(b"two"), &tree.root(), 8));
    }

    #[test]
    fn insert_delete_insert_same_key() {
        let mut tree = SparseTree::new(8);
        let key = [0x42; 32];
        tree.insert(&key, b"first");
        let root1 = tree.root();
        tree.delete(&key);
        tree.insert(&key, b"first");
        // Re-inserting same key+value should produce same root.
        assert_eq!(tree.root(), root1);
    }

    #[test]
    fn deterministic_roots() {
        // Insertion order shouldn't matter for the same final state.
        let k1 = [0u8; 32];
        let k2 = [0xFF; 32];

        let mut tree_a = SparseTree::new(8);
        tree_a.insert(&k1, b"one");
        tree_a.insert(&k2, b"two");

        let mut tree_b = SparseTree::new(8);
        tree_b.insert(&k2, b"two");
        tree_b.insert(&k1, b"one");

        assert_eq!(tree_a.root(), tree_b.root());
    }

    #[test]
    fn key_bit_extraction() {
        let mut key = [0u8; 32];
        key[0] = 0b1010_0000;
        assert!(key_bit(&key, 0));   // MSB = 1
        assert!(!key_bit(&key, 1));  // 0
        assert!(key_bit(&key, 2));   // 1
        assert!(!key_bit(&key, 3));  // 0
    }

    #[test]
    fn mask_key_basic() {
        let key = [0xFF; 32];
        let masked = mask_key(&key, 4);
        assert_eq!(masked[0], 0xF0); // top 4 bits
        assert_eq!(masked[1], 0x00);
    }

    #[test]
    fn sentinel_table_consistency() {
        let s = sentinel_table(4);
        assert_eq!(s.len(), 5); // levels 0..=4
        // Each level builds on the previous.
        assert_eq!(s[1], hash_node(&s[0], &s[0], false));
        assert_eq!(s[2], hash_node(&s[1], &s[1], false));
        assert_eq!(s[3], hash_node(&s[2], &s[2], false));
        assert_eq!(s[4], hash_node(&s[3], &s[3], true)); // root level
    }
}
