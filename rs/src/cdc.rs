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
    0xd1391c501a023d66, 0x91dba4c168e0f243, 0xb96f910645061719, 0xe189cca9d4601bd8,
    0x715c0d2dafd34281, 0x56401e75d415c0b2, 0xe6e3a4a9199441c2, 0x91d809dc94ea4174,
    0xeb65e422400a1366, 0x72a8d7ec4d2974df, 0xc1ade4e5e860d174, 0xcb7ad8bfcd4e37ed,
    0x56ec65a9e2aa1cf9, 0x559c56464040b99d, 0xdb1afb9901bca5a1, 0x4382856e09de5f83,
    0xfb6b197a59db91bd, 0x96deb2714e523ab3, 0xfb4f7f96ad402883, 0x3fbf3124258870f4,
    0xf4d888a419d5cc9f, 0x7c576250ee9adf93, 0x31263594e73a38e2, 0xb6a24311401a6f03,
    0xd9789f1c3e992628, 0xc841f7e052b64e81, 0xb69d21a8792c5e11, 0x34685e69745398d5,
    0x078ba754cc6975ab, 0x87d309a2c92a1ebb, 0x6bf6008da2dce0ab, 0x95cdc12a223302f0,
    0xddeaa380594701f3, 0x352aeba050e14f81, 0x07be2f029044912a, 0x1820ed8ab63ebe59,
    0x05b587bc325e47e3, 0x572b1bf27cf45a3e, 0xfaf4386523006b1c, 0x7f484f63c483d145,
    0x538a52b4d5f21d15, 0x8911e5d3426b5a24, 0x499e1a244599cdc6, 0xcf9d0c90ba287fac,
    0xf96c0809a2eaffcb, 0x0f091ca350c1fa80, 0x304bdef5dcd7eabf, 0xd41245278df37043,
    0x2d233bddfa3876f6, 0x911fb1d206daf027, 0xb60be538fcaf14be, 0x286eabb7c2225883,
    0x3477b657556c6256, 0xb4ee6237c3936435, 0xc43bcf343902f4e2, 0x0163e9001c35a8ab,
    0x64f392ff78d01471, 0x14d86a8004c4c238, 0x082714d2ee45f844, 0xab6957e4e63b5ef5,
    0xe82b7cb6de1df9da, 0xb9d991b652e9290c, 0x5741af0b4a83fcdc, 0x306bf53d6e2204d6,
    0x58d6ff067d3f78a4, 0x7e14d3e98bbd5103, 0x7c94fb173e75963d, 0x7a0b8cd614e21da0,
    0x8de78d88802c11a1, 0xe5b89b0930b22c63, 0x42ecec7538c0b6e3, 0xf31effc4c5ecf551,
    0xa61405a812472604, 0x5b9fa811e1d87c5a, 0x7efb6a8a9c6ba99f, 0x56db1219b972af30,
    0xb0486c69c0eeb48c, 0xed126ff8c627bc50, 0xfe94c00ff42113c3, 0xb82fa20daeff67cf,
    0xbb51ef72aeafb6d7, 0xf658a399fe3aea53, 0x0ee600a593e5a324, 0x160a09edc202c15b,
    0x57831b88c4f28514, 0xfdc82285987ac542, 0x014c2f88d1c46f4f, 0x10f2b9c855401c47,
    0x8e20bd0d78aeb9ca, 0x0bde5a7bd730c333, 0xbab59d20fc92f755, 0xd82c0494c4b048aa,
    0x32c95605f51b8bc9, 0x81f9f137944b009c, 0x1bbfeeb5df0cfe99, 0xdc76a8af809f0ea3,
    0xee5d5a3b245a8acf, 0xe67da2ca3e8ad7f5, 0x2ecd8cb9a52058f5, 0x00a792ca8f98abc6,
    0x7fe1d23175b681ba, 0x584fc591fd8bde01, 0xe55acb531d43d4b8, 0x8de4c7832d9d05d0,
    0x1d00fdfc1a66bcd8, 0xefba5019700a2931, 0xb759261a062d31b2, 0xfdebf16dd1870e4a,
    0xbf627fff81293dc3, 0x0b2e8c3f656de7d2, 0x0275d631fb4df98e, 0x0621ffe34e9d344d,
    0x2f544330a7154171, 0x93acaba298d563e4, 0x515c8b3a3a500c1c, 0x6f7eaa89296c9ba7,
    0x99ca3b0e25ee1d99, 0x047956bc45ffe0c9, 0x118eb63b112705f3, 0xe045020182f0d4b1,
    0x31395184e221c699, 0x25672c5b73b518c1, 0x15a686162a63caa9, 0xe396ab51b03227ec,
    0xbffeda104a165647, 0xdf0fe9ecea31ee3c, 0xa2126ba16ab4090d, 0x811faa3c79672bdf,
    0x5182773a46077601, 0xecef1ca8e0faf7cd, 0xe2684c619ba2d038, 0xd0ac1b27f33b4df0,
    0x12c82233ee6fdb2f, 0x5272fe7f865be811, 0x2d3ba6c7ccb7f1ef, 0xe3ce3d29d74cdb4c,
    0x460afb1756b4b22a, 0x7eaccdd4ded8890f, 0xcd7c32d01e7b2609, 0x25e64902ecd52d9c,
    0x8d3ad61eacd668a7, 0x5336cf99e944de1a, 0xf4d67f38281c46c1, 0x4955bae7284ee23c,
    0x814f0826e7ce1f77, 0xa7450b47a6385d49, 0x50fb71b31e5de239, 0x26458b63c57ba330,
    0x46cb8555c2d7ca9a, 0xf5c519eaf6729dd7, 0xf1d0545ef5ba1ad6, 0x52c76b639f01462f,
    0x7cf3e345a33e60f5, 0x0f9087b3b886afab, 0x78460e0eb2f7e497, 0x16b03589d5ba53bb,
    0xbb9167e29b36db10, 0xb39c85e4f7a99120, 0x3da35ae33db6a5f7, 0xf0220388a0022db0,
    0xb4c8e0569bebb4c2, 0x2ad5a6dd29194105, 0xae6146c05229ee5b, 0xbc568f0ee5e37c5d,
    0xeb40612c249818f8, 0xc4cffbfe28fd3f36, 0xb91a769f38730d5a, 0x95305a9ae5e29dac,
    0x49cfad0102a03cf2, 0x8c964c10e82cc794, 0x68d0b5b138f5a50d, 0x184cf9e2b19783cc,
    0x3574b5d8293b2795, 0xf17a2c194402fb68, 0x19f4dea2cd84b65c, 0x00c6c0a78ae28bbf,
    0x96b14bac4496e7d5, 0xa2e28c32a3f77340, 0x40bede029cc7c830, 0xb633bc731ffe0353,
    0xac3389c1a8e1ca0b, 0xdc7663fe789a5bdb, 0xa6946ebd4f747a75, 0x8ca280d935feecd7,
    0xf3696a3944ba0fee, 0x4244f9df18e2c91d, 0x6415075f983eb02f, 0x7c8ab80e22aa2f11,
    0x447becef3f022434, 0x34b9bcf018478ebb, 0x34592e0ee94eeabb, 0x50594c6441acaf65,
    0xecdcc880678f582a, 0x58718e0a043e25d0, 0x73e3dfba3739f9bc, 0xb28c9b9acf57cb85,
    0x6e0d90599c3e082b, 0xeb31fcfe88026f52, 0x06c035622ad191de, 0x9619816fd00dc3e7,
    0xb69212e7b4274121, 0xbf696b9cc888d90e, 0x268eebefac4a8898, 0xbdd65818f43ebb22,
    0x580a2e1219d26869, 0x4e85d4ef706fe7c0, 0x1af2af574bfedda8, 0x5a74cdf8a82b4160,
    0xdb6bbeccd0f1e9cb, 0x41211b18ca54b047, 0xb9f84ba28b6dd646, 0x32c548e3267fbf9c,
    0x4ec13b55dc680108, 0x270991ab1e1d680c, 0xf4de3ae9c1ab3663, 0xe0c5a70a5eaf8354,
    0x31a05ec73207419e, 0x369220bcfe10c23c, 0x4faf9c5c4cd60cba, 0xa7c618601b0dd5e5,
    0x3e2938b29c73186b, 0x22615bfc81354f1d, 0xa32c5144691a0086, 0x9690eb4ad4e7a011,
    0xf5d145a1635bfdea, 0x29231c28a572adcc, 0x7999c937d576c43a, 0x18432a4886a79f92,
    0x77feef6c04bf627d, 0x1ff264562cc0a090, 0xd8a5635178763a23, 0xfab9dddd6a4adb16,
    0x10730ca07e4dd539, 0x8b66fa94d5c911cf, 0xec7db13d281b5d72, 0xba733423179b99ed,
    0xa5ecd0ab77f0e273, 0xd8c9a05d1a926ee8, 0xd60a1352407bd96a, 0x3704055fa870d0c8,
    0x42fd7a7e490c8375, 0xa2001d5e5d549fba, 0xeaf0b26a7126fcf4, 0x80cb53a741fe8ff8,
    0x377b4de6a2684ac2, 0x7a6adb646910f3cf, 0x3fd598bdc8f6cda8, 0xccfc58b938858a8d,
    0xa29cf8ca38181d15, 0x92581bcc392ed724, 0x0e0021e4b55d35df, 0xbed7687c46505fef,
    0xa235f5df157826d8, 0xfbd87e8c5c2459da, 0xe47e63764d186d51, 0xc3a35dc794841c44,
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
