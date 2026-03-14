//! Low-level primitives for BAO-style Merkle tree construction.
//!
//! Low-level BAO-tree primitives: `chunk_cv` for hashing
//! leaf data into a chaining value, and `parent_cv` for combining two
//! child chaining values into a parent node.

use p3_goldilocks::Goldilocks;

use crate::encoding::{bytes_to_cv, hash_to_bytes};
use crate::params::{self, OUTPUT_ELEMENTS, RATE, WIDTH};
use crate::sponge::Hash;

/// Flags encoded in the capacity for BAO operations.
const FLAG_ROOT: u64 = 1 << 0;
const FLAG_PARENT: u64 = 1 << 1;
const FLAG_CHUNK: u64 = 1 << 2;

/// Capacity index for BAO flags.
const CAPACITY_FLAGS_IDX: usize = RATE + 1; // state[9]

/// Capacity index for chunk counter (position in the file).
const CAPACITY_COUNTER_IDX: usize = RATE; // state[8]

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
}
