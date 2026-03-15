//! Verified streaming encode and decode.
//!
//! Provides content-verified streaming using the hemera hash tree.
//! The combined format interleaves parent hash pairs with leaf data
//! in pre-order, enabling incremental verification.
//!
//! # Format
//!
//! **Combined (pre-order):**
//! ```text
//! [8 bytes: data_len as LE u64]
//! [pre-order traversal of tree]
//!   parent → left_hash ‖ right_hash   (2 × OUTPUT_BYTES)
//!   leaf   → raw chunk data            (≤ CHUNK_SIZE bytes)
//! ```
//!
//! **Outboard:**
//! ```text
//! [8 bytes: data_len as LE u64]
//! [pre-order parent hash pairs only]
//! ```

use alloc::vec::Vec;

use crate::params::{CHUNK_SIZE, OUTPUT_BYTES};
use crate::sponge::Hash;
use crate::tree::{hash_leaf, hash_node, num_chunks};

/// Size of a serialized hash pair (left ‖ right).
const PAIR_SIZE: usize = OUTPUT_BYTES * 2;

/// Header size: 8-byte LE data length.
const HEADER_SIZE: usize = 8;

/// Errors during verified decoding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    /// Stream ended before all data was read.
    Truncated,
    /// A hash did not match the expected value.
    HashMismatch,
}

impl core::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Truncated => f.write_str("truncated stream"),
            Self::HashMismatch => f.write_str("hash verification failed"),
        }
    }
}

// ── Encode ──────────────────────────────────────────────────────

/// Encode data into the combined pre-order format.
///
/// Returns `(root_hash, encoded_bytes)`.
#[allow(unknown_lints, rs_no_vec)]
pub fn encode(data: &[u8]) -> (Hash, Vec<u8>) {
    let n = num_chunks(data.len()) as usize;

    if n <= 1 {
        let root = hash_leaf(data, 0, true);
        let mut out = Vec::with_capacity(HEADER_SIZE + data.len());
        out.extend_from_slice(&(data.len() as u64).to_le_bytes());
        out.extend_from_slice(data);
        return (root, out);
    }

    // Upper bound: header + (n-1) hash pairs + data
    let cap = HEADER_SIZE + (n - 1) * PAIR_SIZE + data.len();
    let mut out = Vec::with_capacity(cap);
    out.extend_from_slice(&(data.len() as u64).to_le_bytes());

    let root = encode_subtree(data, 0, n, true, &mut out);
    (root, out)
}

/// Recursively encode a subtree in pre-order.
///
/// At each parent node, reserves space for the hash pair, recurses into
/// children, then fills the pair in. Leaves emit raw chunk data.
fn encode_subtree(
    data: &[u8],
    offset: usize,
    count: usize,
    is_root: bool,
    out: &mut Vec<u8>,
) -> Hash {
    debug_assert!(count > 0);

    if count == 1 {
        let start = offset * CHUNK_SIZE;
        let end = (start + CHUNK_SIZE).min(data.len());
        let chunk = &data[start..end];
        out.extend_from_slice(chunk);
        return hash_leaf(chunk, offset as u64, false);
    }

    let split = left_subtree_chunks(count);

    // Reserve slot for this node's hash pair (pre-order: parent before children).
    let pair_start = out.len();
    out.extend_from_slice(&[0u8; PAIR_SIZE]);

    let left = encode_subtree(data, offset, split, false, out);
    let right = encode_subtree(data, offset + split, count - split, false, out);

    // Fill in the reserved slot.
    out[pair_start..pair_start + OUTPUT_BYTES].copy_from_slice(left.as_ref());
    out[pair_start + OUTPUT_BYTES..pair_start + PAIR_SIZE].copy_from_slice(right.as_ref());

    hash_node(&left, &right, is_root)
}

// ── Decode ──────────────────────────────────────────────────────

/// Decode and verify a combined pre-order stream.
///
/// Returns the verified data, or an error if the stream is truncated
/// or any hash does not match.
#[allow(unknown_lints, rs_no_vec)]
pub fn decode(encoded: &[u8], expected_root: &Hash) -> Result<Vec<u8>, DecodeError> {
    if encoded.len() < HEADER_SIZE {
        return Err(DecodeError::Truncated);
    }

    let data_len = u64::from_le_bytes(encoded[..HEADER_SIZE].try_into().unwrap()) as usize;
    let n = if data_len == 0 { 1 } else { (data_len + CHUNK_SIZE - 1) / CHUNK_SIZE };
    let mut pos = HEADER_SIZE;

    if n <= 1 {
        // Single chunk: just the raw data after the header.
        if encoded.len() < HEADER_SIZE + data_len {
            return Err(DecodeError::Truncated);
        }
        let chunk = &encoded[HEADER_SIZE..HEADER_SIZE + data_len];
        let cv = hash_leaf(chunk, 0, true);
        if cv != *expected_root {
            return Err(DecodeError::HashMismatch);
        }
        return Ok(chunk.to_vec());
    }

    let mut out = Vec::with_capacity(data_len);
    decode_subtree(encoded, &mut pos, 0, n, true, expected_root, data_len, &mut out)?;

    Ok(out)
}

/// Recursively decode and verify a subtree.
fn decode_subtree(
    encoded: &[u8],
    pos: &mut usize,
    offset: usize,
    count: usize,
    is_root: bool,
    expected: &Hash,
    data_len: usize,
    out: &mut Vec<u8>,
) -> Result<(), DecodeError> {
    debug_assert!(count > 0);

    if count == 1 {
        let start = offset * CHUNK_SIZE;
        let chunk_len = CHUNK_SIZE.min(data_len.saturating_sub(start));
        if *pos + chunk_len > encoded.len() {
            return Err(DecodeError::Truncated);
        }
        let chunk = &encoded[*pos..*pos + chunk_len];
        *pos += chunk_len;

        let cv = hash_leaf(chunk, offset as u64, false);
        if cv != *expected {
            return Err(DecodeError::HashMismatch);
        }
        out.extend_from_slice(chunk);
        return Ok(());
    }

    // Read the hash pair.
    if *pos + PAIR_SIZE > encoded.len() {
        return Err(DecodeError::Truncated);
    }
    let left_hash = read_hash(encoded, pos);
    let right_hash = read_hash(encoded, pos);

    // Verify parent.
    let parent = hash_node(&left_hash, &right_hash, is_root);
    if parent != *expected {
        return Err(DecodeError::HashMismatch);
    }

    let split = left_subtree_chunks(count);
    decode_subtree(encoded, pos, offset, split, false, &left_hash, data_len, out)?;
    decode_subtree(encoded, pos, offset + split, count - split, false, &right_hash, data_len, out)?;

    Ok(())
}

// ── Outboard ────────────────────────────────────────────────────

/// Compute the outboard (hash tree without data) for the given data.
///
/// Returns `(root_hash, outboard_bytes)`. The outboard contains an 8-byte
/// LE size header followed by parent hash pairs in pre-order.
#[allow(unknown_lints, rs_no_vec)]
pub fn outboard(data: &[u8]) -> (Hash, Vec<u8>) {
    let n = num_chunks(data.len()) as usize;

    if n <= 1 {
        let root = hash_leaf(data, 0, true);
        let mut out = Vec::with_capacity(HEADER_SIZE);
        out.extend_from_slice(&(data.len() as u64).to_le_bytes());
        return (root, out);
    }

    let num_parents = n - 1;
    let mut out = Vec::with_capacity(HEADER_SIZE + num_parents * PAIR_SIZE);
    out.extend_from_slice(&(data.len() as u64).to_le_bytes());

    let root = outboard_subtree(data, 0, n, true, &mut out);
    (root, out)
}

/// Recursively serialize parent hash pairs in pre-order.
fn outboard_subtree(
    data: &[u8],
    offset: usize,
    count: usize,
    is_root: bool,
    out: &mut Vec<u8>,
) -> Hash {
    debug_assert!(count > 0);

    if count == 1 {
        let start = offset * CHUNK_SIZE;
        let end = (start + CHUNK_SIZE).min(data.len());
        return hash_leaf(&data[start..end], offset as u64, false);
    }

    let split = left_subtree_chunks(count);

    let pair_start = out.len();
    out.extend_from_slice(&[0u8; PAIR_SIZE]);

    let left = outboard_subtree(data, offset, split, false, out);
    let right = outboard_subtree(data, offset + split, count - split, false, out);

    out[pair_start..pair_start + OUTPUT_BYTES].copy_from_slice(left.as_ref());
    out[pair_start + OUTPUT_BYTES..pair_start + PAIR_SIZE].copy_from_slice(right.as_ref());

    hash_node(&left, &right, is_root)
}

/// Decode and verify data using a separate outboard.
///
/// The outboard must have been produced by [`outboard()`]. The `data` is
/// the original (unencoded) content. Returns `Ok(())` if verification
/// passes.
pub fn verify_outboard(data: &[u8], ob: &[u8], expected_root: &Hash) -> Result<(), DecodeError> {
    if ob.len() < HEADER_SIZE {
        return Err(DecodeError::Truncated);
    }

    let data_len = u64::from_le_bytes(ob[..HEADER_SIZE].try_into().unwrap()) as usize;
    if data.len() != data_len {
        return Err(DecodeError::HashMismatch);
    }

    let n = if data_len == 0 { 1 } else { (data_len + CHUNK_SIZE - 1) / CHUNK_SIZE };

    if n <= 1 {
        let cv = hash_leaf(data, 0, true);
        return if cv == *expected_root { Ok(()) } else { Err(DecodeError::HashMismatch) };
    }

    let mut pos = HEADER_SIZE;
    verify_outboard_subtree(data, ob, &mut pos, 0, n, true, expected_root)
}

/// Recursively verify outboard hash pairs against data.
fn verify_outboard_subtree(
    data: &[u8],
    ob: &[u8],
    pos: &mut usize,
    offset: usize,
    count: usize,
    is_root: bool,
    expected: &Hash,
) -> Result<(), DecodeError> {
    debug_assert!(count > 0);

    if count == 1 {
        let start = offset * CHUNK_SIZE;
        let end = (start + CHUNK_SIZE).min(data.len());
        let cv = hash_leaf(&data[start..end], offset as u64, false);
        return if cv == *expected { Ok(()) } else { Err(DecodeError::HashMismatch) };
    }

    if *pos + PAIR_SIZE > ob.len() {
        return Err(DecodeError::Truncated);
    }
    let left_hash = read_hash(ob, pos);
    let right_hash = read_hash(ob, pos);

    let parent = hash_node(&left_hash, &right_hash, is_root);
    if parent != *expected {
        return Err(DecodeError::HashMismatch);
    }

    let split = left_subtree_chunks(count);
    verify_outboard_subtree(data, ob, pos, offset, split, false, &left_hash)?;
    verify_outboard_subtree(data, ob, pos, offset + split, count - split, false, &right_hash)?;

    Ok(())
}

// ── Helpers ─────────────────────────────────────────────────────

/// Left subtree size for a left-balanced binary tree with `count` leaves.
fn left_subtree_chunks(count: usize) -> usize {
    debug_assert!(count > 1);
    1 << (usize::BITS - (count - 1).leading_zeros() - 1)
}

/// Read a hash from the buffer and advance the position.
fn read_hash(buf: &[u8], pos: &mut usize) -> Hash {
    let mut arr = [0u8; OUTPUT_BYTES];
    arr.copy_from_slice(&buf[*pos..*pos + OUTPUT_BYTES]);
    *pos += OUTPUT_BYTES;
    Hash::from_bytes(arr)
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::vec;
    use super::*;
    use crate::tree::root_hash;

    #[test]
    fn encode_decode_empty() {
        let (root, encoded) = encode(b"");
        assert_eq!(root, root_hash(b""));
        let decoded = decode(&encoded, &root).unwrap();
        assert!(decoded.is_empty());
    }

    #[test]
    fn encode_decode_small() {
        let data = b"hello world";
        let (root, encoded) = encode(data);
        assert_eq!(root, root_hash(data));
        let decoded = decode(&encoded, &root).unwrap();
        assert_eq!(&decoded, data);
    }

    #[test]
    fn encode_decode_exact_chunk() {
        let data = vec![0x42u8; CHUNK_SIZE];
        let (root, encoded) = encode(&data);
        assert_eq!(root, root_hash(&data));
        let decoded = decode(&encoded, &root).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn encode_decode_two_chunks() {
        let data = vec![0xAB; CHUNK_SIZE + 1];
        let (root, encoded) = encode(&data);
        assert_eq!(root, root_hash(&data));
        let decoded = decode(&encoded, &root).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn encode_decode_three_chunks() {
        let data = vec![0xCD; CHUNK_SIZE * 3];
        let (root, encoded) = encode(&data);
        assert_eq!(root, root_hash(&data));
        let decoded = decode(&encoded, &root).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn encode_decode_many_chunks() {
        let data = vec![0xEF; CHUNK_SIZE * 17 + 999];
        let (root, encoded) = encode(&data);
        assert_eq!(root, root_hash(&data));
        let decoded = decode(&encoded, &root).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn decode_wrong_root_fails() {
        let data = b"test data";
        let (_, encoded) = encode(data);
        let wrong = Hash::from_bytes([0xFF; OUTPUT_BYTES]);
        assert_eq!(decode(&encoded, &wrong), Err(DecodeError::HashMismatch));
    }

    #[test]
    fn decode_truncated_header() {
        assert_eq!(decode(b"short", &Hash::from_bytes([0; OUTPUT_BYTES])), Err(DecodeError::Truncated));
    }

    #[test]
    fn decode_truncated_data() {
        let data = vec![0x42; CHUNK_SIZE * 2];
        let (root, encoded) = encode(&data);
        // Truncate the encoded data
        let truncated = &encoded[..encoded.len() - 100];
        assert_eq!(decode(truncated, &root), Err(DecodeError::Truncated));
    }

    #[test]
    fn decode_tampered_data_fails() {
        let data = vec![0x42; CHUNK_SIZE * 2];
        let (root, mut encoded) = encode(&data);
        // Tamper with a data byte (after the header and hash pair)
        let tamper_pos = HEADER_SIZE + PAIR_SIZE + 10;
        encoded[tamper_pos] ^= 0xFF;
        assert_eq!(decode(&encoded, &root), Err(DecodeError::HashMismatch));
    }

    #[test]
    fn decode_tampered_hash_pair_fails() {
        let data = vec![0x42; CHUNK_SIZE * 2];
        let (root, mut encoded) = encode(&data);
        // Tamper with a hash byte in the pair
        encoded[HEADER_SIZE + 5] ^= 0xFF;
        assert_eq!(decode(&encoded, &root), Err(DecodeError::HashMismatch));
    }

    // ── Outboard tests ──────────────────────────────────────────

    #[test]
    fn outboard_single_chunk() {
        let data = b"small data";
        let (root, ob) = outboard(data);
        assert_eq!(root, root_hash(data));
        // Outboard for single chunk: just the header
        assert_eq!(ob.len(), HEADER_SIZE);
        verify_outboard(data, &ob, &root).unwrap();
    }

    #[test]
    fn outboard_multi_chunk() {
        let data = vec![0xAB; CHUNK_SIZE * 4];
        let (root, ob) = outboard(&data);
        assert_eq!(root, root_hash(&data));
        // 4 chunks → 3 parent nodes → 3 hash pairs
        assert_eq!(ob.len(), HEADER_SIZE + 3 * PAIR_SIZE);
        verify_outboard(&data, &ob, &root).unwrap();
    }

    #[test]
    fn outboard_verify_wrong_data_fails() {
        let data = vec![0x42; CHUNK_SIZE * 2];
        let (root, ob) = outboard(&data);
        let wrong = vec![0xFF; CHUNK_SIZE * 2];
        assert_eq!(verify_outboard(&wrong, &ob, &root), Err(DecodeError::HashMismatch));
    }

    #[test]
    fn outboard_verify_wrong_root_fails() {
        let data = vec![0x42; CHUNK_SIZE * 2];
        let (_, ob) = outboard(&data);
        let wrong = Hash::from_bytes([0xFF; OUTPUT_BYTES]);
        assert_eq!(verify_outboard(&data, &ob, &wrong), Err(DecodeError::HashMismatch));
    }

    #[test]
    fn outboard_root_matches_encode_root() {
        let data = vec![0xCD; CHUNK_SIZE * 7 + 500];
        let (encode_root, _) = encode(&data);
        let (outboard_root, _) = outboard(&data);
        assert_eq!(encode_root, outboard_root);
    }

    // ── Roundtrip property tests ────────────────────────────────

    #[test]
    fn encode_decode_roundtrip_sizes() {
        for size in [0, 1, 100, CHUNK_SIZE - 1, CHUNK_SIZE, CHUNK_SIZE + 1,
                     CHUNK_SIZE * 2, CHUNK_SIZE * 3 + 7, CHUNK_SIZE * 8,
                     CHUNK_SIZE * 16 + 1] {
            let data = vec![0x77u8; size];
            let (root, encoded) = encode(&data);
            let decoded = decode(&encoded, &root).unwrap();
            assert_eq!(decoded, data, "roundtrip failed for size {size}");
        }
    }

    #[test]
    fn outboard_verify_sizes() {
        for size in [0, 1, 100, CHUNK_SIZE, CHUNK_SIZE + 1,
                     CHUNK_SIZE * 5, CHUNK_SIZE * 16 + 1] {
            let data = vec![0x88u8; size];
            let (root, ob) = outboard(&data);
            verify_outboard(&data, &ob, &root).unwrap();
        }
    }
}
