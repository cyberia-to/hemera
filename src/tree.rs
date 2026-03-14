//! BAO-style Merkle tree construction.
//!
//! `chunk_cv` hashes leaf data into a chaining value.
//! `parent_cv` combines two child chaining values into a parent node.

use p3_goldilocks::Goldilocks;

use crate::encoding::{bytes_to_cv, hash_to_bytes};
use crate::params::{self, OUTPUT_ELEMENTS, RATE, WIDTH};
use crate::sponge::Hash;

/// Flags encoded in the capacity for BAO operations.
const FLAG_ROOT: u64 = 1 << 0;
const FLAG_PARENT: u64 = 1 << 1;
const FLAG_CHUNK: u64 = 1 << 2;

/// Capacity index for chunk counter (position in the file).
const CAPACITY_COUNTER_IDX: usize = RATE; // state[8]

/// Capacity index for BAO flags.
const CAPACITY_FLAGS_IDX: usize = RATE + 1; // state[9]

/// Capacity index for namespace lower bound (NMT only).
const CAPACITY_NS_MIN_IDX: usize = RATE + 4; // state[12]

/// Capacity index for namespace upper bound (NMT only).
const CAPACITY_NS_MAX_IDX: usize = RATE + 5; // state[13]

/// Compute the chaining value for a leaf chunk.
///
/// The `counter` is the chunk's position index within the file (0-based),
/// used for ordering in BAO tree construction. The `is_root` flag
/// domain-separates root finalization (single-chunk inputs) from interior
/// finalization.
pub fn chunk_cv(chunk: &[u8], counter: u64, is_root: bool) -> Hash {
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
pub fn parent_cv(left: &Hash, right: &Hash, is_root: bool) -> Hash {
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
/// Extends `parent_cv` with namespace bounds committed in capacity:
/// `state[12] = ns_min`, `state[13] = ns_max`. When both are zero this
/// reduces to `parent_cv`. See spec §4.6.4.
pub fn nmt_parent_cv(
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::OUTPUT_BYTES;

    #[test]
    fn parent_cv_non_commutative() {
        let left = Hash::from_bytes([1u8; OUTPUT_BYTES]);
        let right = Hash::from_bytes([2u8; OUTPUT_BYTES]);
        let lr = parent_cv(&left, &right, false);
        let rl = parent_cv(&right, &left, false);
        assert_ne!(lr, rl);
    }

    #[test]
    fn parent_cv_root_differs() {
        let left = Hash::from_bytes([1u8; OUTPUT_BYTES]);
        let right = Hash::from_bytes([2u8; OUTPUT_BYTES]);
        let non_root = parent_cv(&left, &right, false);
        let root = parent_cv(&left, &right, true);
        assert_ne!(non_root, root);
    }

    #[test]
    fn chunk_cv_root_differs() {
        let data = b"chunk data";
        let non_root = chunk_cv(data, 0, false);
        let root = chunk_cv(data, 0, true);
        assert_ne!(non_root, root);
    }

    #[test]
    fn chunk_cv_counter_differs() {
        let data = b"chunk data";
        let c0 = chunk_cv(data, 0, false);
        let c1 = chunk_cv(data, 1, false);
        assert_ne!(c0, c1);
    }

    #[test]
    fn parent_cv_deterministic() {
        let left = Hash::from_bytes([0xAA; OUTPUT_BYTES]);
        let right = Hash::from_bytes([0xBB; OUTPUT_BYTES]);
        let h1 = parent_cv(&left, &right, false);
        let h2 = parent_cv(&left, &right, false);
        assert_eq!(h1, h2);
    }

    #[test]
    fn chunk_cv_different_data() {
        let h1 = chunk_cv(b"data1", 0, false);
        let h2 = chunk_cv(b"data2", 0, false);
        assert_ne!(h1, h2);
    }

    #[test]
    fn chunk_cv_empty() {
        let h = chunk_cv(b"", 0, false);
        assert_ne!(h.as_bytes(), &[0u8; OUTPUT_BYTES]);
    }

    #[test]
    fn chunk_cv_vs_plain_hash() {
        // chunk_cv should differ from a plain hash of the same data
        // because of the CHUNK flag domain separation.
        let data = b"test data";
        let plain = crate::sponge::Hasher::new().update(data).finalize();
        let cv = chunk_cv(data, 0, false);
        assert_ne!(plain, cv);
    }

    // ── Multi-level tree tests ─────────────────────────────────────

    #[test]
    fn two_chunk_tree() {
        // Simulate a 2-chunk file: left + right → root parent
        let left_cv = chunk_cv(b"chunk 0 data", 0, false);
        let right_cv = chunk_cv(b"chunk 1 data", 1, false);
        let root = parent_cv(&left_cv, &right_cv, true);
        assert_ne!(root.as_bytes(), &[0u8; OUTPUT_BYTES]);

        // Deterministic
        let root2 = parent_cv(&left_cv, &right_cv, true);
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
        let c0 = chunk_cv(b"chunk0", 0, false);
        let c1 = chunk_cv(b"chunk1", 1, false);
        let c2 = chunk_cv(b"chunk2", 2, false);
        let c3 = chunk_cv(b"chunk3", 3, false);

        let p01 = parent_cv(&c0, &c1, false);
        let p23 = parent_cv(&c2, &c3, false);
        let root = parent_cv(&p01, &p23, true);

        assert_ne!(root.as_bytes(), &[0u8; OUTPUT_BYTES]);

        // Permuting children changes the root
        let p01_swapped = parent_cv(&c1, &c0, false);
        assert_ne!(p01, p01_swapped);

        let root_swapped = parent_cv(&p01_swapped, &p23, true);
        assert_ne!(root, root_swapped);
    }

    #[test]
    fn single_chunk_root_vs_multi_chunk() {
        // A single-chunk file has is_root=true
        // A multi-chunk file's first chunk has is_root=false
        let data = b"single chunk";
        let single_root = chunk_cv(data, 0, true);
        let multi_first = chunk_cv(data, 0, false);
        assert_ne!(single_root, multi_first);
    }

    #[test]
    fn parent_cv_identical_children() {
        // Even with identical children, parent_cv should produce a non-trivial hash
        let cv = chunk_cv(b"duplicate", 0, false);
        let parent = parent_cv(&cv, &cv, false);
        assert_ne!(parent.as_bytes(), &[0u8; OUTPUT_BYTES]);
        // Parent should differ from either child
        assert_ne!(parent, cv);
    }

    #[test]
    fn chunk_cv_large_counter() {
        // Large counter values should work and produce different results
        let cv_max = chunk_cv(b"data", u64::MAX, false);
        let cv_zero = chunk_cv(b"data", 0, false);
        assert_ne!(cv_max, cv_zero);
    }

    #[test]
    fn chunk_cv_large_data() {
        // Chunk with > 1 rate block of data
        let data = vec![0xAB; 1000];
        let cv = chunk_cv(&data, 0, false);
        assert_ne!(cv.as_bytes(), &[0u8; OUTPUT_BYTES]);

        // Deterministic
        let cv2 = chunk_cv(&data, 0, false);
        assert_eq!(cv, cv2);
    }

    #[test]
    fn parent_cv_non_root_vs_root_with_identical_children() {
        let cv = Hash::from_bytes([0x42; OUTPUT_BYTES]);
        let non_root = parent_cv(&cv, &cv, false);
        let root = parent_cv(&cv, &cv, true);
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
    fn nmt_parent_cv_zero_ns_matches_parent_cv() {
        let left = Hash::from_bytes([1u8; OUTPUT_BYTES]);
        let right = Hash::from_bytes([2u8; OUTPUT_BYTES]);
        let standard = parent_cv(&left, &right, false);
        let nmt_zero = nmt_parent_cv(&left, &right, 0, 0, false);
        assert_eq!(standard, nmt_zero);
    }

    #[test]
    fn nmt_parent_cv_ns_differs_from_plain() {
        let left = Hash::from_bytes([1u8; OUTPUT_BYTES]);
        let right = Hash::from_bytes([2u8; OUTPUT_BYTES]);
        let plain = parent_cv(&left, &right, false);
        let with_ns = nmt_parent_cv(&left, &right, 10, 20, false);
        assert_ne!(plain, with_ns);
    }

    #[test]
    fn nmt_parent_cv_different_ns_ranges() {
        let left = Hash::from_bytes([1u8; OUTPUT_BYTES]);
        let right = Hash::from_bytes([2u8; OUTPUT_BYTES]);
        let h1 = nmt_parent_cv(&left, &right, 10, 20, false);
        let h2 = nmt_parent_cv(&left, &right, 10, 30, false);
        assert_ne!(h1, h2);
    }

    #[test]
    fn nmt_parent_cv_root_differs() {
        let left = Hash::from_bytes([1u8; OUTPUT_BYTES]);
        let right = Hash::from_bytes([2u8; OUTPUT_BYTES]);
        let non_root = nmt_parent_cv(&left, &right, 5, 15, false);
        let root = nmt_parent_cv(&left, &right, 5, 15, true);
        assert_ne!(non_root, root);
    }

    #[test]
    fn nmt_parent_cv_non_commutative() {
        let left = Hash::from_bytes([1u8; OUTPUT_BYTES]);
        let right = Hash::from_bytes([2u8; OUTPUT_BYTES]);
        let lr = nmt_parent_cv(&left, &right, 5, 15, false);
        let rl = nmt_parent_cv(&right, &left, 5, 15, false);
        assert_ne!(lr, rl);
    }

    #[test]
    fn nmt_capacity_indices_no_overlap() {
        assert_eq!(CAPACITY_NS_MIN_IDX, 12);
        assert_eq!(CAPACITY_NS_MAX_IDX, 13);
        // No overlap with counter (8), flags (9), msg_length (10), domain (11)
        assert!(CAPACITY_NS_MIN_IDX > 11);
    }
}
