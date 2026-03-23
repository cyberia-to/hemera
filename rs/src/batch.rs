// ---
// tags: hemera, rust
// crystal-type: source
// crystal-domain: comp
// ---
//! Batch inclusion proofs for Hemera Merkle trees.
//!
//! A batch proof for k leaves deduplicates shared sibling hashes,
//! producing a proof smaller than k individual proofs combined.
//! Construction and verification follow the same recursive tree walk,
//! so sibling ordering is deterministic.

use crate::params::CHUNK_SIZE;
use crate::sponge::Hash;
use crate::tree::{hash_leaf, hash_node, merge_range, num_chunks};

/// A batch Merkle inclusion proof for multiple leaves.
///
/// Siblings are stored in depth-first left-to-right order matching
/// the recursive tree walk. The verifier consumes them in the same order.
#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(unknown_lints, rs_no_vec)]
pub struct BatchInclusionProof {
    /// Sorted leaf (chunk) indices.
    pub indices: alloc::vec::Vec<u64>,
    /// Deduplicated sibling hashes in tree-walk order.
    pub siblings: alloc::vec::Vec<Hash>,
    /// Total number of chunks in the tree.
    pub num_chunks: u64,
    /// Expected root hash.
    pub root: Hash,
}

/// Generate a batch inclusion proof for the given chunk indices.
///
/// `indices` must be sorted and within `[0, num_chunks)`.
/// Returns the root hash and the batch proof.
#[allow(unknown_lints, rs_no_vec)]
pub fn prove_batch(data: &[u8], indices: &[u64]) -> (Hash, BatchInclusionProof) {
    let n = num_chunks(data.len());
    assert!(!indices.is_empty(), "indices must be non-empty");
    for (i, &idx) in indices.iter().enumerate() {
        assert!(idx < n, "index {idx} out of range for {n} chunks");
        if i > 0 {
            assert!(indices[i - 1] < idx, "indices must be sorted and unique");
        }
    }

    if n == 1 {
        let root = hash_leaf(data, 0, true);
        return (
            root,
            BatchInclusionProof {
                indices: alloc::vec::Vec::from(indices),
                siblings: alloc::vec::Vec::new(),
                num_chunks: n,
                root,
            },
        );
    }

    let mut siblings = alloc::vec::Vec::new();
    let root = collect_siblings(data, 0, n as usize, indices, true, &mut siblings);

    (
        root,
        BatchInclusionProof {
            indices: alloc::vec::Vec::from(indices),
            siblings,
            num_chunks: n,
            root,
        },
    )
}

/// Recursive tree walk that collects only the sibling hashes a verifier cannot compute.
#[allow(unknown_lints, rs_no_vec)]
fn collect_siblings(
    data: &[u8],
    offset: usize,
    count: usize,
    targets: &[u64],
    is_root: bool,
    siblings: &mut alloc::vec::Vec<Hash>,
) -> Hash {
    // Find which targets fall in this subtree [offset, offset+count).
    let lo = targets.partition_point(|&t| (t as usize) < offset);
    let hi = targets.partition_point(|&t| (t as usize) < offset + count);
    let local = &targets[lo..hi];

    // No targets in this subtree — compute and return the hash.
    // The prover will emit this hash so the verifier can consume it.
    if local.is_empty() {
        return merge_range(data, offset, count, is_root);
    }

    // Single chunk that IS a target — return its leaf hash.
    if count == 1 {
        let start = offset * CHUNK_SIZE;
        let end = (start + CHUNK_SIZE).min(data.len());
        return hash_leaf(&data[start..end], offset as u64, false);
    }

    let split = 1 << (usize::BITS - (count - 1).leading_zeros() - 1);
    let left_has = local.iter().any(|&t| (t as usize) < offset + split);
    let right_has = local.iter().any(|&t| (t as usize) >= offset + split);

    let (left_hash, right_hash) = if left_has && right_has {
        // Both subtrees have targets — recurse both, no sibling needed.
        let l = collect_siblings(data, offset, split, targets, false, siblings);
        let r = collect_siblings(data, offset + split, count - split, targets, false, siblings);
        (l, r)
    } else if left_has {
        // Only left has targets — right subtree becomes a sibling.
        let r = merge_range(data, offset + split, count - split, false);
        siblings.push(r);
        let l = collect_siblings(data, offset, split, targets, false, siblings);
        (l, r)
    } else {
        // Only right has targets — left subtree becomes a sibling.
        let l = merge_range(data, offset, split, false);
        siblings.push(l);
        let r = collect_siblings(data, offset + split, count - split, targets, false, siblings);
        (l, r)
    };

    hash_node(&left_hash, &right_hash, is_root)
}

/// Verify a batch inclusion proof against the provided chunk data.
///
/// `chunks[i]` is the data for `proof.indices[i]`. Returns `true`
/// if the reconstructed root matches `proof.root` and all siblings
/// are consumed.
#[allow(unknown_lints, rs_no_vec)]
pub fn verify_batch(chunks: &[&[u8]], proof: &BatchInclusionProof) -> bool {
    if chunks.len() != proof.indices.len() {
        return false;
    }
    if proof.num_chunks == 0 {
        return false;
    }

    if proof.num_chunks == 1 {
        if chunks.len() != 1 {
            return false;
        }
        let root = hash_leaf(chunks[0], 0, true);
        return root == proof.root;
    }

    let mut cursor = 0usize;
    let result = verify_subtree(
        chunks,
        &proof.indices,
        &proof.siblings,
        &mut cursor,
        0,
        proof.num_chunks as usize,
        true,
    );

    match result {
        Some(root) => root == proof.root && cursor == proof.siblings.len(),
        None => false,
    }
}

/// Recursive verification: rebuild the root from chunks + siblings.
fn verify_subtree(
    chunks: &[&[u8]],
    indices: &[u64],
    siblings: &[Hash],
    cursor: &mut usize,
    offset: usize,
    count: usize,
    is_root: bool,
) -> Option<Hash> {
    let lo = indices.partition_point(|&t| (t as usize) < offset);
    let hi = indices.partition_point(|&t| (t as usize) < offset + count);
    let local_count = hi - lo;

    // No targets — consume next sibling.
    if local_count == 0 {
        if *cursor >= siblings.len() {
            return None;
        }
        let h = siblings[*cursor];
        *cursor += 1;
        return Some(h);
    }

    // Single chunk that is a target.
    if count == 1 {
        // Find which chunk index this corresponds to.
        let chunk_pos = lo; // indices[lo] == offset as u64
        if chunk_pos >= chunks.len() {
            return None;
        }
        return Some(hash_leaf(chunks[chunk_pos], offset as u64, false));
    }

    let split = 1 << (usize::BITS - (count - 1).leading_zeros() - 1);
    let left_has = indices[lo..hi].iter().any(|&t| (t as usize) < offset + split);
    let right_has = indices[lo..hi].iter().any(|&t| (t as usize) >= offset + split);

    let (left_hash, right_hash) = if left_has && right_has {
        let l = verify_subtree(chunks, indices, siblings, cursor, offset, split, false)?;
        let r = verify_subtree(
            chunks, indices, siblings, cursor,
            offset + split, count - split, false,
        )?;
        (l, r)
    } else if left_has {
        // Right subtree is a sibling (consumed first, matching construction order).
        if *cursor >= siblings.len() {
            return None;
        }
        let r = siblings[*cursor];
        *cursor += 1;
        let l = verify_subtree(chunks, indices, siblings, cursor, offset, split, false)?;
        (l, r)
    } else {
        // Left subtree is a sibling.
        if *cursor >= siblings.len() {
            return None;
        }
        let l = siblings[*cursor];
        *cursor += 1;
        let r = verify_subtree(
            chunks, indices, siblings, cursor,
            offset + split, count - split, false,
        )?;
        (l, r)
    };

    Some(hash_node(&left_hash, &right_hash, is_root))
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::vec;
    use super::*;
    use crate::tree::root_hash;

    #[test]
    fn batch_single_leaf_matches_single_proof() {
        let data = vec![0x42u8; CHUNK_SIZE * 4];
        let root = root_hash(&data);

        for i in 0..4u64 {
            let (r, proof) = prove_batch(&data, &[i]);
            assert_eq!(r, root);
            let start = i as usize * CHUNK_SIZE;
            let end = start + CHUNK_SIZE;
            assert!(verify_batch(&[&data[start..end]], &proof));
        }
    }

    #[test]
    fn batch_two_adjacent_leaves() {
        let data = vec![0xAB; CHUNK_SIZE * 4];
        let root = root_hash(&data);
        let (r, proof) = prove_batch(&data, &[0, 1]);
        assert_eq!(r, root);

        let c0 = &data[..CHUNK_SIZE];
        let c1 = &data[CHUNK_SIZE..CHUNK_SIZE * 2];
        assert!(verify_batch(&[c0, c1], &proof));

        // Adjacent leaves share a parent — fewer siblings than 2 individual proofs.
        // Individual: 2 * 2 = 4 siblings. Batch should have fewer.
        assert!(proof.siblings.len() < 4);
    }

    #[test]
    fn batch_two_distant_leaves() {
        let data = vec![0xCD; CHUNK_SIZE * 8];
        let root = root_hash(&data);
        let (r, proof) = prove_batch(&data, &[0, 7]);
        assert_eq!(r, root);

        let c0 = &data[..CHUNK_SIZE];
        let c7 = &data[CHUNK_SIZE * 7..];
        assert!(verify_batch(&[c0, c7], &proof));
    }

    #[test]
    fn batch_all_leaves() {
        let data = vec![0xEF; CHUNK_SIZE * 4];
        let root = root_hash(&data);
        let (r, proof) = prove_batch(&data, &[0, 1, 2, 3]);
        assert_eq!(r, root);

        let chunks: std::vec::Vec<&[u8]> = (0..4)
            .map(|i| &data[i * CHUNK_SIZE..(i + 1) * CHUNK_SIZE])
            .collect();
        assert!(verify_batch(&chunks, &proof));

        // All leaves present — no siblings needed.
        assert_eq!(proof.siblings.len(), 0);
    }

    #[test]
    fn batch_contiguous_range() {
        let data = vec![0x77; CHUNK_SIZE * 8];
        let root = root_hash(&data);
        let indices: std::vec::Vec<u64> = (0..4).collect();
        let (r, proof) = prove_batch(&data, &indices);
        assert_eq!(r, root);

        let chunks: std::vec::Vec<&[u8]> = (0..4)
            .map(|i| &data[i * CHUNK_SIZE..(i + 1) * CHUNK_SIZE])
            .collect();
        assert!(verify_batch(&chunks, &proof));
    }

    #[test]
    fn batch_wrong_data_fails() {
        let data = vec![0x42u8; CHUNK_SIZE * 4];
        let (_, proof) = prove_batch(&data, &[0, 1]);
        let wrong = vec![0xFF; CHUNK_SIZE];
        assert!(!verify_batch(&[&wrong, &data[CHUNK_SIZE..CHUNK_SIZE * 2]], &proof));
    }

    #[test]
    fn batch_wrong_root_fails() {
        let data = vec![0x42u8; CHUNK_SIZE * 4];
        let (_, mut proof) = prove_batch(&data, &[0]);
        proof.root = Hash::from_bytes([0xFF; 64]);
        assert!(!verify_batch(&[&data[..CHUNK_SIZE]], &proof));
    }

    #[test]
    fn batch_single_chunk_file() {
        let data = b"small data";
        let root = root_hash(data);
        let (r, proof) = prove_batch(data, &[0]);
        assert_eq!(r, root);
        assert!(verify_batch(&[data.as_slice()], &proof));
    }

    #[test]
    fn batch_three_chunks_all() {
        let data = vec![0xAA; CHUNK_SIZE * 3];
        let root = root_hash(&data);
        let (r, proof) = prove_batch(&data, &[0, 1, 2]);
        assert_eq!(r, root);

        let chunks: std::vec::Vec<&[u8]> = (0..3)
            .map(|i| {
                let start = i * CHUNK_SIZE;
                let end = (start + CHUNK_SIZE).min(data.len());
                &data[start..end]
            })
            .collect();
        assert!(verify_batch(&chunks, &proof));
        assert_eq!(proof.siblings.len(), 0);
    }

    #[test]
    fn batch_savings_over_individual() {
        // 2^10 = 1024 chunks, prove 32 contiguous leaves.
        // Individual: 32 * 10 = 320 siblings.
        // Batch: ~10 + 31 = 41 siblings.
        let data = vec![0x55; CHUNK_SIZE * 1024];
        let root = root_hash(&data);
        let indices: std::vec::Vec<u64> = (0..32).collect();
        let (r, proof) = prove_batch(&data, &indices);
        assert_eq!(r, root);
        assert!(proof.siblings.len() < 320);

        let chunks: std::vec::Vec<&[u8]> = (0..32)
            .map(|i| &data[i * CHUNK_SIZE..(i + 1) * CHUNK_SIZE])
            .collect();
        assert!(verify_batch(&chunks, &proof));
    }

    #[test]
    #[should_panic(expected = "sorted and unique")]
    fn batch_unsorted_panics() {
        let data = vec![0x42; CHUNK_SIZE * 4];
        prove_batch(&data, &[2, 1]);
    }

    #[test]
    #[should_panic(expected = "out of range")]
    fn batch_out_of_range_panics() {
        let data = vec![0x42; CHUNK_SIZE * 4];
        prove_batch(&data, &[4]);
    }

    #[test]
    fn batch_extra_siblings_rejected() {
        let data = vec![0x42u8; CHUNK_SIZE * 4];
        let (_, mut proof) = prove_batch(&data, &[0]);
        proof.siblings.push(Hash::from_bytes([0xAA; 64]));
        assert!(!verify_batch(&[&data[..CHUNK_SIZE]], &proof));
    }
}
