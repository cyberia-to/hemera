//! Poseidon2 hash over the Goldilocks field (Hemera parameters).
//!
//! # WARNING
//!
//! **This is novel, unaudited cryptography.** The parameter set, sponge
//! construction, and self-bootstrapping round constant generation have not
//! been reviewed by third-party cryptographers. Do not use in production
//! systems where cryptographic guarantees are required. Use at your own risk.
//!
//! ---
//!
//! This crate provides a streaming hash API backed by the Poseidon2
//! algebraic hash function operating over the Goldilocks prime field
//! (p = 2^64 - 2^32 + 1).
//!
//! # Hemera Parameters
//!
//! - **Field**: Goldilocks (p = 2^64 - 2^32 + 1)
//! - **State width**: t = 16
//! - **Full rounds**: R_F = 8
//! - **Partial rounds**: R_P = 64
//! - **S-box degree**: d = 7 (x^7)
//! - **Rate**: 8 elements (56 input bytes per block)
//! - **Capacity**: 8 elements
//! - **Output**: 8 elements (64 bytes)
//! - **Padding**: 0x01 || 0x00*
//! - **Encoding**: little-endian canonical
//!
//! # Examples
//!
//! ```
//! use cyber_hemera::{hash, derive_key};
//!
//! let digest = hash(b"hello world");
//! println!("{digest}");
//!
//! let key = derive_key("my app v1", b"key material");
//! ```

#[cfg(test)]
mod bootstrap;
mod constants;
mod encoding;
pub mod tree;
mod params;
mod sponge;

#[cfg(feature = "gpu")]
pub mod gpu;

// Re-export all Hemera parameters so downstream crates never hardcode them.
pub use params::{
    CAPACITY, CHUNK_SIZE, COLLISION_BITS, OUTPUT_BYTES, OUTPUT_ELEMENTS, RATE, RATE_BYTES,
    ROUNDS_F, ROUNDS_P, SBOX_DEGREE, WIDTH,
};
pub use sponge::{Hash, Hasher, OutputReader};

/// Hash the input bytes and return a 64-byte digest.
pub fn hash(input: &[u8]) -> Hash {
    let mut hasher = Hasher::new();
    hasher.update(input);
    hasher.finalize()
}

/// Hash the input bytes with a key.
pub fn keyed_hash(key: &[u8; OUTPUT_BYTES], input: &[u8]) -> Hash {
    let mut hasher = Hasher::new_keyed(key);
    hasher.update(input);
    hasher.finalize()
}

/// Derive a key from a context string and key material.
///
/// This is a two-phase operation:
/// 1. Hash the context string with domain separation
/// 2. Use the context hash to seed a second hasher that absorbs the key material
pub fn derive_key(context: &str, key_material: &[u8]) -> [u8; OUTPUT_BYTES] {
    let ctx_hasher = Hasher::new_derive_key_context(context);
    let ctx_hash = ctx_hasher.finalize();
    let mut material_hasher = Hasher::new_derive_key_material(&ctx_hash);
    material_hasher.update(key_material);
    let result = material_hasher.finalize();
    *result.as_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_basic() {
        let h = hash(b"hello");
        assert_ne!(h.as_bytes(), &[0u8; OUTPUT_BYTES]);
    }

    #[test]
    fn hash_deterministic() {
        let h1 = hash(b"test");
        let h2 = hash(b"test");
        assert_eq!(h1, h2);
    }

    #[test]
    fn hash_different_inputs() {
        assert_ne!(hash(b""), hash(b"a"));
        assert_ne!(hash(b"a"), hash(b"b"));
        assert_ne!(hash(b"ab"), hash(b"ba"));
    }

    #[test]
    fn hash_matches_streaming() {
        let data = b"streaming consistency test with enough data to cross boundaries!!";
        let direct = hash(data);
        let streamed = {
            let mut h = Hasher::new();
            h.update(&data[..10]);
            h.update(&data[10..]);
            h.finalize()
        };
        assert_eq!(direct, streamed);
    }

    #[test]
    fn keyed_hash_differs_from_plain() {
        let data = b"test";
        assert_ne!(hash(data), keyed_hash(&[0u8; OUTPUT_BYTES], data));
    }

    #[test]
    fn keyed_hash_different_keys() {
        let data = b"test";
        let h1 = keyed_hash(&[0u8; OUTPUT_BYTES], data);
        let h2 = keyed_hash(&[1u8; OUTPUT_BYTES], data);
        assert_ne!(h1, h2);
    }

    #[test]
    fn derive_key_basic() {
        let key = derive_key("my context", b"material");
        assert_ne!(key, [0u8; OUTPUT_BYTES]);
    }

    #[test]
    fn derive_key_differs_from_hash() {
        let data = b"material";
        let h = hash(data);
        let k = derive_key("context", data);
        assert_ne!(h.as_bytes(), &k);
    }

    #[test]
    fn derive_key_different_contexts() {
        let k1 = derive_key("context A", b"material");
        let k2 = derive_key("context B", b"material");
        assert_ne!(k1, k2);
    }

    #[test]
    fn derive_key_different_materials() {
        let k1 = derive_key("context", b"material A");
        let k2 = derive_key("context", b"material B");
        assert_ne!(k1, k2);
    }

    #[test]
    fn xof_extends_hash() {
        let mut xof = Hasher::new().update(b"xof test").finalize_xof();
        let mut out = [0u8; OUTPUT_BYTES * 2];
        xof.fill(&mut out);
        // First OUTPUT_BYTES match finalize.
        let h = hash(b"xof test");
        assert_eq!(&out[..OUTPUT_BYTES], h.as_bytes());
    }

    #[test]
    fn large_input() {
        let data = vec![0x42u8; 10_000];
        let h = hash(&data);
        assert_ne!(h.as_bytes(), &[0u8; OUTPUT_BYTES]);

        // Streaming equivalence.
        let mut hasher = Hasher::new();
        for chunk in data.chunks(137) {
            hasher.update(chunk);
        }
        assert_eq!(h, hasher.finalize());
    }

    #[test]
    fn hash_empty() {
        let h = hash(b"");
        assert_ne!(h.as_bytes(), &[0u8; OUTPUT_BYTES]);
    }

    #[test]
    fn hash_single_byte_avalanche() {
        // Each single-byte input should produce a wildly different hash
        let hashes: Vec<_> = (0..=255u8).map(|b| hash(&[b])).collect();
        for i in 0..256 {
            for j in (i + 1)..256 {
                assert_ne!(hashes[i], hashes[j], "collision at bytes {i} and {j}");
            }
        }
    }

    #[test]
    fn keyed_hash_empty_input() {
        let h = keyed_hash(&[0u8; OUTPUT_BYTES], b"");
        assert_ne!(h.as_bytes(), &[0u8; OUTPUT_BYTES]);
    }

    #[test]
    fn derive_key_long_context() {
        // Context longer than one rate block
        let long_ctx = "a]".repeat(100);
        let k = derive_key(&long_ctx, b"material");
        assert_ne!(k, [0u8; OUTPUT_BYTES]);
    }

    #[test]
    fn derive_key_long_material() {
        // Material longer than one rate block
        let material = vec![0x42u8; 1000];
        let k = derive_key("ctx", &material);
        assert_ne!(k, [0u8; OUTPUT_BYTES]);
    }
}

/// Property-based tests using proptest.
#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn hash_is_deterministic(data in proptest::collection::vec(any::<u8>(), 0..500)) {
            prop_assert_eq!(hash(&data), hash(&data));
        }

        #[test]
        fn streaming_matches_oneshot(data in proptest::collection::vec(any::<u8>(), 0..500)) {
            let oneshot = hash(&data);
            let mut hasher = Hasher::new();
            // Feed in random-ish chunk sizes
            let mut pos = 0;
            let mut chunk_size = 1;
            while pos < data.len() {
                let end = (pos + chunk_size).min(data.len());
                hasher.update(&data[pos..end]);
                pos = end;
                chunk_size = (chunk_size * 3 + 1) % 71; // pseudo-random sizes
                if chunk_size == 0 { chunk_size = 1; }
            }
            prop_assert_eq!(oneshot, hasher.finalize());
        }

        #[test]
        fn xof_prefix_matches_finalize(data in proptest::collection::vec(any::<u8>(), 0..200)) {
            let hash_result = hash(&data);
            let mut xof = {
                let mut h = Hasher::new();
                h.update(&data);
                h.finalize_xof()
            };
            let mut xof_bytes = [0u8; OUTPUT_BYTES];
            xof.fill(&mut xof_bytes);
            prop_assert_eq!(hash_result.as_bytes(), &xof_bytes);
        }

        #[test]
        fn keyed_hash_differs_from_plain(
            data in proptest::collection::vec(any::<u8>(), 1..200),
            key in proptest::collection::vec(any::<u8>(), OUTPUT_BYTES..=OUTPUT_BYTES),
        ) {
            let key_arr: [u8; OUTPUT_BYTES] = key.try_into().unwrap();
            let plain = hash(&data);
            let keyed = keyed_hash(&key_arr, &data);
            prop_assert_ne!(plain, keyed);
        }

        #[test]
        fn clone_consistency(
            prefix in proptest::collection::vec(any::<u8>(), 0..100),
            suffix in proptest::collection::vec(any::<u8>(), 0..100),
        ) {
            let mut h1 = Hasher::new();
            h1.update(&prefix);
            let mut h2 = h1.clone();
            h1.update(&suffix);
            h2.update(&suffix);
            prop_assert_eq!(h1.finalize(), h2.finalize());
        }
    }
}
