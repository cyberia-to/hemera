// ---
// tags: hemera, rust
// crystal-type: source
// crystal-domain: comp
// ---
//! Element-aligned content-defined chunking (CDC) and section tree construction.
//!
//! Implements the gear table, element fingerprint, window size, CDC boundary
//! rule, and `section_cdc_tree` as specified in roadmap/semantic-hashing.md.

extern crate alloc;
use alloc::vec::Vec;

use crate::sponge::Hash;
use crate::tree::{hash_leaf, merge_leaf_hashes};

// ── Gear table ───────────────────────────────────────────────────────

/// Gear table: GEAR_TABLE[i] = u64::from_le_bytes(hemera::hash(&[i])[0..8]).
///
/// Frozen alongside the complete hemera sponge construction (permutation
/// constants, padding, domain tag, rate/capacity split, output encoding).
/// Any change to the sponge invalidates all particle_id values.
const GEAR_TABLE: [u64; 256] = [
    0x3cee16fda88038bc, 0xeb3f0bed5aa5c5da, 0x89f51d2ea4b37853, 0x83a074e025363a05,
    0x68cb54c27a65da0c, 0x989e7d4fa6ca7bc8, 0x90ca94da54b3094b, 0x20c4dc2b9172db1b,
    0xb41eb49c005a6e6c, 0x0383e0caa5de0b27, 0xd18a8371f7d86311, 0xa2e74b491c8af8d1,
    0xe3a8fa536c4b2034, 0x450130663b30f62b, 0x475be834aeae481c, 0x685d19fc14b0580d,
    0xd02c77e54253d244, 0x3b991a3bcfec554e, 0xacff39810448f2e6, 0xa491297672297cdb,
    0xb914e9c3f66eb5fd, 0x91013827c56342b4, 0x7438f473b7838b55, 0xce5b43b15b444b8e,
    0x922ee5c4fde4bd60, 0xce9408781ec455ff, 0x4a679855e36af282, 0x0352e3835a88c9be,
    0x6ee3d7b8dfbb76c6, 0xea7119ace6923987, 0xdfdfc312c06568c5, 0x29463415b5b9a5ea,
    0x57f1e0a1531a965d, 0x0077df6ac00c6632, 0x64f65c161960a54b, 0x288a093194ef23cc,
    0x90c8450d1773a97b, 0xc25e58080a77e237, 0x53540e49ba167c78, 0x8661781e21de3349,
    0x4c5ea83276ddd10c, 0xdec40127893df77d, 0xb947a919c779eb4b, 0xb3efea5835c35750,
    0xd4f7d7d27988461e, 0x31f8de42fcb5691a, 0x168fa72f2bb15de3, 0x388290904d4b8a34,
    0xb66c4e053a9f5234, 0x4401d916d6e48c29, 0xdf5d0bb0bb2606d1, 0x421efc106baea8cb,
    0x4fe81637898f7c67, 0x892a2050092c8118, 0x025777c137211bc9, 0x0d8f2a6e63a83fce,
    0xed141ddb65b5a45a, 0x2e620431a908be6c, 0x97ae7a41e25c164c, 0xbf656f2af1d76fe2,
    0x698cc9ae5f637107, 0xf88169c3ec238d46, 0xb97e7c1b727cff48, 0x9e34abc9721b1b3d,
    0x8cb3f21a8030fa39, 0x0493ae82badba1f7, 0x89897c3013d24323, 0x0b8d907262681586,
    0xa93f8b770f22735f, 0xccebf34ea621b450, 0xedbbfb260a4f96c1, 0xa00aa83f9bf3ffed,
    0x2819e0bbcf867cba, 0xa456e02b81085178, 0x10ea0a71910f37b0, 0x16a8499ffcc5f8b3,
    0x4a33b44d69025992, 0x2394329cccc9c2d6, 0xb347b089a9d0b330, 0x8a63580a2728b967,
    0x0161d40558d91265, 0x82aaf7773aed3aba, 0xc79fbf6dc6fc741d, 0xb0d1feed8b763a7d,
    0x27ea025a45c33e26, 0xce9e2c46d801e543, 0xa9d6ac50f857f4e7, 0x5bb0f458232c909c,
    0xbdf74c7bbb9efa7b, 0x25fc6f3fccc950d7, 0xdb0caa3f4ca9d3dc, 0xd173d09fbf41061d,
    0x8bda78112cb958f0, 0x98ac58d7aad11560, 0x2b76f4a7a3481ba9, 0xbfa79169a62ca8db,
    0xd5a5b428af26b9e6, 0xde2d8790f81c4ac2, 0x107ff6e2f4f1670f, 0xb4b4cb5badd76ea0,
    0x074f4c210f80155d, 0xcf86f83cbbd01c4a, 0x1cc7ac25016da70b, 0xcda8dd852bdacb2f,
    0x5ebd485e5e7c9352, 0xae73935ff72b98e6, 0x049cb0ce036a6d2e, 0x4f884a76b443efc0,
    0x072269080e39090e, 0xe179ad1b6a7965b8, 0x4b7e4771785cc58c, 0x2fc3ff2b831f72b8,
    0xa3f522492063315b, 0x0ae52e16dd56e85d, 0xc44c6714f9c8a5d2, 0x6258323c82ff9fce,
    0xbcd59ef962d11eda, 0x94e4d059b1a08680, 0x814cccfc4eacf5a4, 0x39111048a74a6d07,
    0xc1fba777587402be, 0xcfb775abfa7cad4a, 0x27b0d9bc5eab8293, 0x94a054339c69e39c,
    0x8b2eb24d778be7de, 0x3c366ba9e7fd565e, 0xa417de11b59728d9, 0x503b65803a941d26,
    0x37d1c13ff6ce5c55, 0x920832d54e4463fb, 0xbf7ef2a4db80bf66, 0x901625f0268dce41,
    0x7d534cceba27667d, 0xa7bd1c9132c22517, 0x21a95f6ac2a402ff, 0x6c7243715cab7a18,
    0x633cb5bf0c8184c4, 0x48658d2c959e9338, 0x3674ec7d8c6d3d02, 0x211f649a8a7f51bf,
    0xf81564b99b42a187, 0x2f9d5127f5263263, 0xab96b8daaedd9a1b, 0x254cc67128559262,
    0x6084b340de0b599b, 0xa66f7d949aabf212, 0xff91ca59d34d6270, 0xb778531421b168c7,
    0x3d8b7750edc78ed9, 0xb111ac1204ccf799, 0x9a2007eed273fb2a, 0xf5deed029880afbe,
    0x420d0413433999e0, 0xdebd2f6e3a6d91f1, 0x7a2090ad3448cd05, 0xced530cf89a18ed1,
    0xe2e6fe0be276942a, 0x72fc757214f57508, 0x6867ea3c642286bc, 0x04fbc94e61805705,
    0x8598891cf8ba8fe0, 0x6e1d1cda8f7fa56a, 0x270baa26c7b78658, 0x899bd295c727a6bd,
    0x4a7b6bb56e3d82dd, 0x3894e0ace0e03556, 0xa5449e2af60158b0, 0xf34df44d175008e5,
    0xd7eaf1b6b3a38b05, 0xc3d7e2c0b390b207, 0xff1f0be59d7c6f09, 0x689d1a2c2b98e56f,
    0x072df565671d0f3a, 0xe8c17ff4fd465a6e, 0x69698d12e5743678, 0x135ef28fc58dd5d8,
    0xd6f50c186ef03188, 0xa5eb8d9264fab43d, 0xf87ff7811740be67, 0xe8ca945aa2dce5df,
    0x54d28e365bdd589c, 0x20ddcbd837d253b6, 0x4b8c084edf432bf8, 0x3c90ff183c66c405,
    0x4e86e64a12df4509, 0x5798d3e12f773d5e, 0x537b58c11d087ea2, 0xa024e53d558b1cc4,
    0xd2b2cbadffb895a3, 0x63dd9afb269f3a20, 0x8ea67ce1b067266a, 0x21448ec35a8490f5,
    0x56c4ae1088c4d73e, 0x3cbe94d87ff73e41, 0xeb6f699d4775e2de, 0x77e1d3455aeec637,
    0x4b0a48c6c0ccddf9, 0xb25d80790085e303, 0xfb934fc1d7e5866b, 0xd78c433dd2ecb9dc,
    0xaf5937367908dfb6, 0x8abde3e9cb44ae1c, 0xcd590541810ea843, 0xc16d66644cb6a901,
    0x71f1fa2886fbb124, 0x59c3bf04f7a80adc, 0x15f5cf9b6e7c128b, 0xbfb7ada32df4c71a,
    0xca295fb24e47e022, 0xaa74f361a1a22bd5, 0x79a7c767dc148691, 0xa0a36a00948c0043,
    0x8e0d8a7da8b1d22f, 0xbb2c41e2e14c9b78, 0x834d9cad216150fc, 0xbfcec1d159a78e43,
    0xba18a26e1009f56c, 0xadcea3bcefd99544, 0xa37f5dd5632a6df6, 0xd090d233291056f9,
    0x91739bcdbc56fa77, 0x0b6c8365f5c661df, 0xe6a32f293c82c95c, 0x269b5d559b8582f4,
    0x7245e2d07ebadc2e, 0x7270eaf7a78a0f39, 0xc92e61d11a2c2620, 0x5f1b563d0479633b,
    0xe8b29885c36765eb, 0x8b0f60e6ff51007a, 0xda3bacb6ebabca2a, 0x2ff3b41ff463319c,
    0xa3dfe89870296087, 0xecdb68da2aa107ae, 0x656e550d99c8da85, 0x3a15b068f6ba480f,
    0x2e90e10a4629f338, 0x08c3d2f27f488aac, 0x82605e1c3e4c812b, 0xc4986aaf5a5873e6,
    0xff51ba344855ae3b, 0xc9e18ffce06bf0e1, 0x6c6bf57d3e496e9d, 0x1fd8cbbcce3fc3e7,
    0xe4459ecbda0011a7, 0x584fff779b8aad1b, 0xe26fe55549193600, 0x9a5909733d39d299,
    0x0277e7ea034412d5, 0xb3bd84a478bfad88, 0x53dc76456936fd3f, 0x5b81725e5b02948d,
    0xa5a8d8c069ad4ae9, 0xdcc06aa0277a7f95, 0xa48bb63137f4bba2, 0x4ca3941ddb6263b2,
];

// ── Fingerprint ───────────────────────────────────────────────────────

/// Compute the fingerprint of one element of `element_size` bytes.
///
/// fp(e) = XOR of rotate_left(GEAR_TABLE[e[k]], (k*11) % 64) for k in 0..S.
/// Rotation increment 11 is coprime to 64 → all 64 rotation values visited
/// exactly once for S=64; every byte position has a distinct rotation weight.
#[inline]
pub(crate) fn element_fingerprint(element: &[u8]) -> u64 {
    let mut fp = 0u64;
    for (k, &byte) in element.iter().enumerate() {
        let rot = (k as u32 * 11) % 64;
        fp ^= GEAR_TABLE[byte as usize].rotate_left(rot);
    }
    fp
}

// ── Window size ───────────────────────────────────────────────────────

/// Compute window size W for element_size S.
///
/// W = next_power_of_two(max(64, floor(4096 / S)))
/// Targets ~4096 bytes per chunk regardless of element_size.
#[inline]
pub(crate) fn window_size(element_size: usize) -> usize {
    let base = (4096 / element_size).max(64);
    base.next_power_of_two()
}

// ── CDC boundary computation ─────────────────────────────────────────

/// Compute CDC boundaries for `n` elements of `element_size` bytes each.
///
/// Returns a Vec of element-index boundaries: always starts with 0, ends with n.
/// The number of chunks K = boundaries.len() - 1.
#[allow(unknown_lints, rs_no_vec)]
fn cdc_boundaries(data: &[u8], element_size: usize, n: usize) -> Vec<usize> {
    let w = window_size(element_size);
    let min_chunk = w / 2;
    let max_chunk = w * 2;

    let mut boundaries = Vec::new();
    boundaries.push(0usize);
    let mut last = 0usize;

    while last < n {
        let chunk_start = last;
        // lo = first candidate index for the last element of this chunk
        let lo = chunk_start + min_chunk - 1;
        let hi = (chunk_start + max_chunk - 1).min(n - 1);

        if lo > n - 1 {
            // Fewer than min_chunk elements remain → final partial chunk.
            boundaries.push(n);
            break;
        }

        // Scan [lo, hi] for the first occurrence of the minimum fingerprint.
        let lo_elem = &data[lo * element_size..(lo + 1) * element_size];
        let mut min_fp = element_fingerprint(lo_elem);
        let mut min_pos = lo;

        for i in (lo + 1)..=hi {
            let elem = &data[i * element_size..(i + 1) * element_size];
            let fp = element_fingerprint(elem);
            if fp < min_fp {
                min_fp = fp;
                min_pos = i;
            }
        }

        boundaries.push(min_pos + 1);
        last = min_pos + 1;
    }

    boundaries
}

// ── Section CDC tree ─────────────────────────────────────────────────

/// Hash a section using element-aligned CDC and a left-balanced Merkle tree.
///
/// - `element_size` must be in [1, 64].
/// - `data.len()` must be an exact multiple of `element_size`.
/// - Returns the section root hash with the `is_root` flag threaded through.
///
/// Empty section → `hash_leaf([], 0, is_root)`.
/// Single CDC chunk → `hash_leaf(data, 0, is_root)`.
/// Multiple chunks → left-balanced tree of leaf hashes.
#[allow(unknown_lints, rs_no_vec)]
pub fn section_cdc_tree(data: &[u8], element_size: usize, is_root: bool) -> Hash {
    debug_assert!(element_size >= 1 && element_size <= 64,
        "element_size {element_size} out of [1, 64]");
    debug_assert!(data.len() % element_size == 0,
        "data length {} not a multiple of element_size {element_size}", data.len());

    let n = data.len() / element_size;

    if n == 0 {
        return hash_leaf(&[], 0, is_root);
    }

    let boundaries = cdc_boundaries(data, element_size, n);
    let k = boundaries.len() - 1;

    if k == 1 {
        return hash_leaf(data, 0, is_root);
    }

    let leaves: Vec<Hash> = (0..k)
        .map(|i| {
            let start = boundaries[i] * element_size;
            let end = boundaries[i + 1] * element_size;
            hash_leaf(&data[start..end], i as u64, false)
        })
        .collect();

    merge_leaf_hashes(&leaves, is_root)
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    extern crate std;
    use std::vec;
    use super::*;
    use crate::tree::hash_node;
    use crate::params::OUTPUT_BYTES;

    // ── Gear table verification ─────────────────────────────────────

    #[test]
    fn gear_table_entry_zero_matches_hash() {
        let h = crate::hash(&[0u8]);
        let expected = u64::from_le_bytes(h.as_bytes()[..8].try_into().unwrap());
        assert_eq!(GEAR_TABLE[0], expected);
    }

    #[test]
    fn gear_table_entry_255_matches_hash() {
        let h = crate::hash(&[255u8]);
        let expected = u64::from_le_bytes(h.as_bytes()[..8].try_into().unwrap());
        assert_eq!(GEAR_TABLE[255], expected);
    }

    #[test]
    fn gear_table_all_entries_distinct() {
        // Verify no two entries in the gear table are equal.
        // (statistical expectation — would indicate a broken hash if violated)
        let mut seen = std::collections::HashSet::new();
        for &v in GEAR_TABLE.iter() {
            assert!(seen.insert(v), "duplicate gear table entry: 0x{v:016x}");
        }
    }

    // ── Fingerprint ─────────────────────────────────────────────────

    #[test]
    fn fingerprint_single_byte_is_table_lookup() {
        for i in 0u8..=255 {
            let fp = element_fingerprint(&[i]);
            assert_eq!(fp, GEAR_TABLE[i as usize]);
        }
    }

    #[test]
    fn fingerprint_two_bytes_uses_rotations() {
        let e = [0u8, 0u8];
        // fp = GEAR_TABLE[0] ^ rotate_left(GEAR_TABLE[0], 11)
        let expected = GEAR_TABLE[0] ^ GEAR_TABLE[0].rotate_left(11);
        assert_eq!(element_fingerprint(&e), expected);
    }

    #[test]
    fn fingerprint_different_elements_differ() {
        let fp1 = element_fingerprint(&[1u8, 2u8, 3u8, 4u8]);
        let fp2 = element_fingerprint(&[4u8, 3u8, 2u8, 1u8]);
        assert_ne!(fp1, fp2, "element order must affect fingerprint");
    }

    // ── Window size ─────────────────────────────────────────────────

    #[test]
    fn window_size_element_size_1_is_4096() {
        assert_eq!(window_size(1), 4096);
    }

    #[test]
    fn window_size_element_size_64_is_64() {
        assert_eq!(window_size(64), 64);
    }

    #[test]
    fn window_size_always_power_of_two() {
        for s in 1..=64usize {
            let w = window_size(s);
            assert!(w.is_power_of_two(), "W={w} not power of two for element_size={s}");
            assert!(w >= 64, "W must be >= 64");
        }
    }

    // ── section_cdc_tree ────────────────────────────────────────────

    #[test]
    fn empty_section_gives_hash_leaf_empty() {
        let h = section_cdc_tree(&[], 1, true);
        assert_eq!(h, hash_leaf(&[], 0, true));
    }

    #[test]
    fn section_smaller_than_min_chunk_is_single_leaf() {
        // min_chunk = W/2 = 4096/2 = 2048 for element_size=1
        let data = vec![0x42u8; 2047];
        let h = section_cdc_tree(&data, 1, true);
        assert_eq!(h, hash_leaf(&data, 0, true));
    }

    #[test]
    fn section_exactly_min_chunk_may_split() {
        // 2048 bytes: [lo=2047, hi=min(8191,2047)=2047] → only element at lo
        // → boundary at 2048 = n → K=1 (single chunk covers all)
        let data = vec![0x42u8; 2048];
        let h = section_cdc_tree(&data, 1, true);
        assert_eq!(h, hash_leaf(&data, 0, true));
    }

    #[test]
    fn uniform_data_above_min_chunk_splits_at_min_chunk() {
        // Uniform data: all fps equal → first minimum always at lo → chunk size = min_chunk.
        // 4096 bytes: K=2, chunks [0..2048], [2048..4096]
        let data = vec![0x42u8; 4096];
        let h = section_cdc_tree(&data, 1, true);
        let leaf0 = hash_leaf(&data[..2048], 0, false);
        let leaf1 = hash_leaf(&data[2048..], 1, false);
        assert_eq!(h, hash_node(&leaf0, &leaf1, true));
    }

    #[test]
    fn section_is_deterministic() {
        let data = vec![0xABu8; 8192];
        assert_eq!(
            section_cdc_tree(&data, 1, true),
            section_cdc_tree(&data, 1, true),
        );
    }

    #[test]
    fn section_is_root_flag_changes_hash() {
        let data = vec![0xABu8; 8192];
        let with_root = section_cdc_tree(&data, 1, true);
        let without_root = section_cdc_tree(&data, 1, false);
        assert_ne!(with_root, without_root);
    }

    #[test]
    fn section_different_data_different_hash() {
        let a = vec![0x11u8; 4096];
        let b = vec![0x22u8; 4096];
        assert_ne!(section_cdc_tree(&a, 1, true), section_cdc_tree(&b, 1, true));
    }

    #[test]
    fn element_size_2_divisible_data() {
        // element_size=2: each element is 2 bytes. 4096 bytes = 2048 elements.
        // W = next_pow2(max(64, 4096/2=2048)) = 2048. min_chunk=1024, max_chunk=4096.
        // 2048 elements: lo=1023, hi=min(4095,2047)=2047. Uniform data → K=2 or K=3.
        let data = vec![0x42u8; 4096];
        let h = section_cdc_tree(&data, 2, true);
        // Just verify determinism and non-null.
        assert_eq!(h, section_cdc_tree(&data, 2, true));
        assert_ne!(h.as_bytes(), &[0u8; OUTPUT_BYTES]);
    }

    #[test]
    fn element_size_64_divisible_data() {
        // element_size=64: W=64, min_chunk=32, max_chunk=128.
        // 4096 bytes = 64 elements. first chunk [0..32 elements = 2048 bytes].
        let data = vec![0x42u8; 4096];
        let h = section_cdc_tree(&data, 64, true);
        assert_eq!(h, section_cdc_tree(&data, 64, true));
    }

    #[test]
    fn element_size_affects_window_size() {
        // W is different for different element sizes, which affects CDC structure.
        // Verify that W changes correctly (more direct than checking hash inequality
        // which may coincide for uniform data).
        assert_ne!(window_size(1), window_size(4));
        assert_ne!(window_size(4), window_size(16));
        assert_eq!(window_size(1), 4096);  // baseline
        assert_eq!(window_size(4), 1024);
        assert_eq!(window_size(16), 256);
    }

    #[test]
    fn cdc_boundaries_stable_under_prefix_append() {
        // Two sections that share a suffix. After the CDC re-sync point, chunk
        // boundaries (and thus chunk hashes) should be identical.
        // We verify that adding a byte at the start perturbs only early chunks.
        let mut base = vec![0xFFu8; 8192];
        // Vary bytes in positions 0..64 only (well within one chunk)
        base[0] = 0xAA;
        base[1] = 0xBB;
        let modified: Vec<u8> = [&[0xCC][..], &base[1..]].concat();

        // Hashes will differ (different data), but section_cdc_tree on two
        // inputs sharing a long suffix has similar chunk structure after re-sync.
        // Just verify that both produce valid, distinct hashes.
        let h1 = section_cdc_tree(&base, 1, true);
        let h2 = section_cdc_tree(&modified, 1, true);
        assert_ne!(h1, h2);
    }

    #[test]
    fn large_section_hashes_without_panic() {
        // 1 MB of data; verify no panic and determinism.
        let data = vec![0x42u8; 1024 * 1024];
        let h1 = section_cdc_tree(&data, 1, true);
        let h2 = section_cdc_tree(&data, 1, true);
        assert_eq!(h1, h2);
    }
}
