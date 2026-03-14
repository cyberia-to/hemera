//! Poseidon2 hash over the Goldilocks field (Hemera parameters).
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
//! use hemera::{hash, derive_key};
//!
//! let digest = hash(b"hello world");
//! println!("{digest}");
//!
//! let key = derive_key("my app v1", b"key material");
//! ```

mod encoding;
pub mod hazmat;
mod params;
mod sponge;

#[cfg(feature = "gpu")]
pub mod gpu;

// Re-export all Hemera parameters so downstream crates never hardcode them.
pub use params::{
    CAPACITY, COLLISION_BITS, OUTPUT_BYTES, OUTPUT_ELEMENTS, RATE, RATE_BYTES, ROUNDS_F, ROUNDS_P,
    SBOX_DEGREE, WIDTH,
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
}
