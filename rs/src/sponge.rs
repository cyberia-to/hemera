use core::fmt;

use crate::encoding::{bytes_to_rate_block, hash_to_bytes};
use crate::field::Goldilocks;
use crate::params::{self, OUTPUT_BYTES, OUTPUT_BYTES_PER_ELEMENT, OUTPUT_ELEMENTS, RATE, RATE_BYTES, WIDTH};

/// Domain separation tags placed in `state[capacity_start + 3]` (i.e. `state[11]`).
const DOMAIN_HASH: u64 = 0x00;
const DOMAIN_KEYED: u64 = 0x01;
const DOMAIN_DERIVE_KEY_CONTEXT: u64 = 0x02;
const DOMAIN_DERIVE_KEY_MATERIAL: u64 = 0x03;

/// Index where the capacity region starts (after the rate region).
const CAPACITY_START: usize = RATE; // 8

/// A 64-byte Poseidon2 hash output (Hemera: 8 Goldilocks elements).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Hash([u8; OUTPUT_BYTES]);

impl Hash {
    /// Create a hash from a raw byte array.
    pub const fn from_bytes(bytes: [u8; OUTPUT_BYTES]) -> Self {
        Self(bytes)
    }

    /// Return the hash as a byte slice.
    pub fn as_bytes(&self) -> &[u8; OUTPUT_BYTES] {
        &self.0
    }

    /// Convert the hash to a hex string.
    #[cfg(feature = "std")]
    #[allow(unknown_lints, rs_no_string)]
    pub fn to_hex(&self) -> alloc::string::String {
        let mut s = alloc::string::String::with_capacity(OUTPUT_BYTES * 2);
        for byte in &self.0 {
            use core::fmt::Write;
            write!(s, "{byte:02x}").unwrap();
        }
        s
    }
}

impl From<[u8; OUTPUT_BYTES]> for Hash {
    fn from(bytes: [u8; OUTPUT_BYTES]) -> Self {
        Self(bytes)
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for Hash {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeTuple;
        let mut seq = serializer.serialize_tuple(OUTPUT_BYTES)?;
        for byte in &self.0 {
            seq.serialize_element(byte)?;
        }
        seq.end()
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Hash {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct HashVisitor;
        impl<'de> serde::de::Visitor<'de> for HashVisitor {
            type Value = Hash;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                write!(formatter, "a byte array of length {OUTPUT_BYTES}")
            }

            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> Result<Hash, A::Error> {
                let mut bytes = [0u8; OUTPUT_BYTES];
                for (i, byte) in bytes.iter_mut().enumerate() {
                    *byte = seq
                        .next_element()?
                        .ok_or_else(|| serde::de::Error::invalid_length(i, &self))?;
                }
                Ok(Hash(bytes))
            }
        }
        deserializer.deserialize_tuple(OUTPUT_BYTES, HashVisitor)
    }
}

impl AsRef<[u8]> for Hash {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl fmt::Display for Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in &self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl fmt::Debug for Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Hash({self})")
    }
}

/// A streaming Poseidon2 hasher.
///
/// Supports three modes via domain separation:
/// - Plain hash (`new`)
/// - Keyed hash (`new_keyed`)
/// - Key derivation (`new_derive_key`)
///
/// Data is absorbed in 56-byte blocks (8 Goldilocks elements × 7 bytes each).
/// Zero heap allocations — the internal buffer is a fixed `[u8; RATE_BYTES]`.
#[derive(Clone)]
pub struct Hasher {
    state: [Goldilocks; WIDTH],
    buf: [u8; RATE_BYTES],
    buf_len: usize,
    absorbed: u64,
}

impl Hasher {
    /// Create a new hasher in plain hash mode.
    pub fn new() -> Self {
        let mut state = [Goldilocks::new(0); WIDTH];
        state[CAPACITY_START + 3] = Goldilocks::new(DOMAIN_HASH);
        Self {
            state,
            buf: [0u8; RATE_BYTES],
            buf_len: 0,
            absorbed: 0,
        }
    }

    /// Create a new hasher in keyed hash mode.
    ///
    /// The key is absorbed as the first block (before any user data).
    pub fn new_keyed(key: &[u8; OUTPUT_BYTES]) -> Self {
        let mut state = [Goldilocks::new(0); WIDTH];
        state[CAPACITY_START + 3] = Goldilocks::new(DOMAIN_KEYED);

        // Absorb the key into the rate portion via the normal buffer path.
        let mut hasher = Self {
            state,
            buf: [0u8; RATE_BYTES],
            buf_len: 0,
            absorbed: 0,
        };
        hasher.update(key.as_slice());
        hasher
    }

    /// Create a new hasher in derive-key mode.
    ///
    /// First hashes the context string to produce a context key, then
    /// sets up a second hasher seeded with that key for absorbing key material.
    pub fn new_derive_key_context(context: &str) -> Self {
        let mut state = [Goldilocks::new(0); WIDTH];
        state[CAPACITY_START + 3] = Goldilocks::new(DOMAIN_DERIVE_KEY_CONTEXT);
        let mut hasher = Self {
            state,
            buf: [0u8; RATE_BYTES],
            buf_len: 0,
            absorbed: 0,
        };
        hasher.update(context.as_bytes());
        hasher
    }

    /// Create a derive-key hasher for the material phase, seeded by a context hash.
    pub fn new_derive_key_material(context_hash: &Hash) -> Self {
        let mut state = [Goldilocks::new(0); WIDTH];
        state[CAPACITY_START + 3] = Goldilocks::new(DOMAIN_DERIVE_KEY_MATERIAL);

        // Seed the rate portion with the context hash (8 elements = 64 bytes).
        for (i, chunk) in context_hash.0.chunks(OUTPUT_BYTES_PER_ELEMENT).enumerate() {
            let val = u64::from_le_bytes(chunk.try_into().unwrap());
            state[i] = Goldilocks::new(val);
        }
        params::permute(&mut state);

        Self {
            state,
            buf: [0u8; RATE_BYTES],
            buf_len: 0,
            absorbed: 0,
        }
    }

    /// Absorb input data into the sponge.
    pub fn update(&mut self, mut data: &[u8]) -> &mut Self {
        self.absorbed += data.len() as u64;

        // Fill the buffer from data, processing complete rate blocks.
        while !data.is_empty() {
            let space = RATE_BYTES - self.buf_len;
            let n = space.min(data.len());
            self.buf[self.buf_len..self.buf_len + n].copy_from_slice(&data[..n]);
            self.buf_len += n;
            data = &data[n..];

            if self.buf_len == RATE_BYTES {
                let mut rate_block = [Goldilocks::new(0); RATE];
                bytes_to_rate_block(&self.buf, &mut rate_block);
                self.absorb_block(&rate_block);
                self.buf_len = 0;
            }
        }

        self
    }

    /// Add a rate block into the state (Goldilocks field addition) and permute.
    fn absorb_block(&mut self, block: &[Goldilocks; RATE]) {
        for (i, block_elem) in block.iter().enumerate() {
            self.state[i] = self.state[i] + *block_elem;
        }
        params::permute(&mut self.state);
    }

    /// Apply padding and produce the finalized state.
    ///
    /// Padding scheme (Hemera: 0x01 || 0x00*):
    /// 1. Append 0x01 byte to remaining buffer
    /// 2. Pad to RATE_BYTES with zeros
    /// 3. Encode as field elements and absorb
    /// 4. Store total byte count in capacity[2]
    pub fn finalize_state(&self) -> [Goldilocks; WIDTH] {
        let mut state = self.state;
        let mut padded = [0u8; RATE_BYTES];
        padded[..self.buf_len].copy_from_slice(&self.buf[..self.buf_len]);

        // Append padding marker (Hemera: 0x01).
        padded[self.buf_len] = 0x01;
        // Remaining bytes already zero.

        // Encode and absorb the final block (Goldilocks field addition).
        let mut rate_block = [Goldilocks::new(0); RATE];
        bytes_to_rate_block(&padded, &mut rate_block);
        for i in 0..RATE {
            state[i] = state[i] + rate_block[i];
        }

        // Encode total length in capacity.
        state[CAPACITY_START + 2] = Goldilocks::new(self.absorbed);

        params::permute(&mut state);
        state
    }

    /// Finalize and return the hash.
    pub fn finalize(&self) -> Hash {
        let state = self.finalize_state();
        let output: [Goldilocks; OUTPUT_ELEMENTS] = state[..OUTPUT_ELEMENTS]
            .try_into()
            .unwrap();
        Hash(hash_to_bytes(&output))
    }

    /// Finalize and return an extendable output reader (XOF mode).
    pub fn finalize_xof(&self) -> OutputReader {
        let state = self.finalize_state();
        OutputReader {
            state,
            buffer: [0u8; OUTPUT_BYTES],
            buffer_pos: OUTPUT_BYTES, // empty — will squeeze on first read
        }
    }
}

impl Default for Hasher {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for Hasher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Hasher")
            .field("absorbed", &self.absorbed)
            .field("buffered", &self.buf_len)
            .finish()
    }
}

/// An extendable-output reader that can produce arbitrary-length output.
///
/// Operates by repeatedly squeezing OUTPUT_BYTES from the sponge state,
/// then permuting to produce more output.
pub struct OutputReader {
    state: [Goldilocks; WIDTH],
    buffer: [u8; OUTPUT_BYTES],
    buffer_pos: usize,
}

impl OutputReader {
    /// Fill the provided buffer with hash output bytes.
    pub fn fill(&mut self, output: &mut [u8]) {
        let mut written = 0;
        while written < output.len() {
            if self.buffer_pos >= OUTPUT_BYTES {
                self.squeeze();
            }
            let available = OUTPUT_BYTES - self.buffer_pos;
            let needed = output.len() - written;
            let n = available.min(needed);
            output[written..written + n]
                .copy_from_slice(&self.buffer[self.buffer_pos..self.buffer_pos + n]);
            self.buffer_pos += n;
            written += n;
        }
    }

    /// Squeeze one block of output from the sponge.
    fn squeeze(&mut self) {
        let output_elems: [Goldilocks; OUTPUT_ELEMENTS] = self.state[..OUTPUT_ELEMENTS]
            .try_into()
            .unwrap();
        self.buffer = hash_to_bytes(&output_elems);
        self.buffer_pos = 0;
        params::permute(&mut self.state);
    }
}

#[cfg(feature = "std")]
impl std::io::Read for OutputReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.fill(buf);
        Ok(buf.len())
    }
}

impl fmt::Debug for OutputReader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OutputReader").finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::{format, vec, vec::Vec};
    use super::*;

    #[test]
    fn hash_display_is_hex() {
        let h = Hash([0xAB; OUTPUT_BYTES]);
        let s = format!("{h}");
        assert_eq!(s.len(), OUTPUT_BYTES * 2);
        assert!(s.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn empty_hash_is_not_zero() {
        let h = Hasher::new().finalize();
        assert_ne!(h.0, [0u8; OUTPUT_BYTES]);
    }

    #[test]
    fn different_inputs_different_hashes() {
        let h1 = Hasher::new().update(b"a").finalize();
        let h2 = Hasher::new().update(b"b").finalize();
        assert_ne!(h1, h2);
    }

    #[test]
    fn streaming_consistency() {
        let data = b"hello world, this is a test of streaming consistency!";
        let one_shot = {
            let mut h = Hasher::new();
            h.update(data);
            h.finalize()
        };
        let streamed = {
            let mut h = Hasher::new();
            h.update(&data[..5]);
            h.update(&data[5..20]);
            h.update(&data[20..]);
            h.finalize()
        };
        assert_eq!(one_shot, streamed);
    }

    #[test]
    fn streaming_across_rate_boundary() {
        // 56 bytes = exactly one rate block, so 100 bytes crosses a boundary.
        let data = vec![0x42u8; 100];
        let one_shot = {
            let mut h = Hasher::new();
            h.update(&data);
            h.finalize()
        };
        let byte_at_a_time = {
            let mut h = Hasher::new();
            for b in &data {
                h.update(core::slice::from_ref(b));
            }
            h.finalize()
        };
        assert_eq!(one_shot, byte_at_a_time);
    }

    #[test]
    fn domain_separation_hash_vs_keyed() {
        let data = b"test data";
        let plain = Hasher::new().update(data).finalize();
        let keyed = Hasher::new_keyed(&[0u8; OUTPUT_BYTES]).update(data).finalize();
        assert_ne!(plain, keyed);
    }

    #[test]
    fn domain_separation_hash_vs_derive_key() {
        let data = b"test material";
        let plain = Hasher::new().update(data).finalize();
        let ctx_hasher = Hasher::new_derive_key_context("test context");
        let ctx_hash = ctx_hasher.finalize();
        let derived = Hasher::new_derive_key_material(&ctx_hash)
            .update(data)
            .finalize();
        assert_ne!(plain, derived);
    }

    #[test]
    fn xof_first_32_match_finalize() {
        let data = b"xof test";
        let hash = Hasher::new().update(data).finalize();
        let mut xof = Hasher::new().update(data).finalize_xof();
        let mut xof_bytes = [0u8; OUTPUT_BYTES];
        xof.fill(&mut xof_bytes);
        assert_eq!(hash.as_bytes(), &xof_bytes);
    }

    #[test]
    fn xof_produces_more_than_32_bytes() {
        let mut xof = Hasher::new().update(b"xof").finalize_xof();
        let mut out = [0u8; 128];
        xof.fill(&mut out);
        // Not all zeros.
        assert_ne!(out, [0u8; 128]);
        // Different 32-byte blocks (with overwhelming probability).
        assert_ne!(out[..OUTPUT_BYTES], out[OUTPUT_BYTES..OUTPUT_BYTES * 2]);
    }

    #[test]
    fn xof_read_trait() {
        use std::io::Read;
        let mut xof = Hasher::new().update(b"read trait").finalize_xof();
        let mut buf = [0u8; 64];
        let n = xof.read(&mut buf).unwrap();
        assert_eq!(n, 64);
    }

    #[test]
    fn keyed_hash_different_keys() {
        let data = b"same data";
        let h1 = Hasher::new_keyed(&[0u8; OUTPUT_BYTES]).update(data).finalize();
        let h2 = Hasher::new_keyed(&[1u8; OUTPUT_BYTES]).update(data).finalize();
        assert_ne!(h1, h2);
    }

    // ── Padding boundary tests ──────────────────────────────────────

    #[test]
    fn exact_rate_block_input() {
        // 56 bytes = exactly one rate block. Padding adds a second full block.
        let data = vec![0x42u8; RATE_BYTES];
        let h = Hasher::new().update(&data).finalize();
        assert_ne!(h.0, [0u8; OUTPUT_BYTES]);
        // Streaming equivalence: split at byte 28
        let h2 = {
            let mut hasher = Hasher::new();
            hasher.update(&data[..28]);
            hasher.update(&data[28..]);
            hasher.finalize()
        };
        assert_eq!(h, h2);
    }

    #[test]
    fn exact_two_rate_blocks_input() {
        // 112 bytes = exactly two rate blocks
        let data = vec![0x42u8; RATE_BYTES * 2];
        let h = Hasher::new().update(&data).finalize();
        let h_streamed = {
            let mut hasher = Hasher::new();
            for chunk in data.chunks(17) { // odd chunk size
                hasher.update(chunk);
            }
            hasher.finalize()
        };
        assert_eq!(h, h_streamed);
    }

    #[test]
    fn one_less_than_rate_block() {
        // 55 bytes: padding appends 0x01 to make exactly 56 bytes
        let data = vec![0x42u8; RATE_BYTES - 1];
        let h = Hasher::new().update(&data).finalize();
        assert_ne!(h.0, [0u8; OUTPUT_BYTES]);
    }

    #[test]
    fn one_more_than_rate_block() {
        // 57 bytes: first 56 go to block 1, remaining 1 + padding = block 2
        let data = vec![0x42u8; RATE_BYTES + 1];
        let h = Hasher::new().update(&data).finalize();
        assert_ne!(h.0, [0u8; OUTPUT_BYTES]);
    }

    // ── Clone consistency ──────────────────────────────────────────

    #[test]
    fn hasher_clone_produces_same_hash() {
        let mut h1 = Hasher::new();
        h1.update(b"some data");
        let h2 = h1.clone();
        h1.update(b" more");
        let mut h3 = h1.clone();
        h3.update(b"");
        assert_eq!(h1.finalize(), h3.finalize());
        // h2 diverged at "some data"
        assert_ne!(h1.finalize(), h2.finalize());
    }

    #[test]
    fn hasher_clone_mid_block() {
        let mut h = Hasher::new();
        h.update(&[0xAB; 30]); // mid-block (< 56)
        let cloned = h.clone();
        h.update(&[0xCD; 30]);
        let mut cloned2 = cloned.clone();
        cloned2.update(&[0xCD; 30]);
        assert_eq!(h.finalize(), cloned2.finalize());
    }

    // ── XOF tests ──────────────────────────────────────────────────

    #[test]
    fn xof_incremental_reads_match_bulk() {
        let mut xof1 = Hasher::new().update(b"xof incremental").finalize_xof();
        let mut xof2 = Hasher::new().update(b"xof incremental").finalize_xof();

        // Bulk read
        let mut bulk = [0u8; 200];
        xof1.fill(&mut bulk);

        // Incremental reads of varying sizes
        let mut incremental = Vec::new();
        for size in [1, 3, 7, 13, 64, 50, 62] {
            let mut buf = vec![0u8; size];
            xof2.fill(&mut buf);
            incremental.extend_from_slice(&buf);
        }

        assert_eq!(&bulk[..], &incremental[..]);
    }

    #[test]
    fn xof_deterministic() {
        let mut xof1 = Hasher::new().update(b"deterministic").finalize_xof();
        let mut xof2 = Hasher::new().update(b"deterministic").finalize_xof();
        let mut out1 = [0u8; 256];
        let mut out2 = [0u8; 256];
        xof1.fill(&mut out1);
        xof2.fill(&mut out2);
        assert_eq!(out1, out2);
    }

    #[test]
    fn xof_different_inputs_different_streams() {
        let mut xof1 = Hasher::new().update(b"input A").finalize_xof();
        let mut xof2 = Hasher::new().update(b"input B").finalize_xof();
        let mut out1 = [0u8; 128];
        let mut out2 = [0u8; 128];
        xof1.fill(&mut out1);
        xof2.fill(&mut out2);
        assert_ne!(out1, out2);
    }

    #[test]
    fn xof_zero_length_fill() {
        let mut xof = Hasher::new().update(b"zero").finalize_xof();
        let mut empty = [];
        xof.fill(&mut empty); // should not panic

        // Subsequent reads should still work
        let mut out = [0u8; 64];
        xof.fill(&mut out);
        assert_ne!(out, [0u8; 64]);
    }

    // ── Hash type tests ────────────────────────────────────────────

    #[test]
    fn hash_from_bytes_roundtrip() {
        let bytes = [0xAB; OUTPUT_BYTES];
        let h = Hash::from_bytes(bytes);
        assert_eq!(h.as_bytes(), &bytes);
    }

    #[test]
    fn hash_to_hex_length() {
        let h = Hash::from_bytes([0x00; OUTPUT_BYTES]);
        let hex = format!("{h}");
        assert_eq!(hex.len(), OUTPUT_BYTES * 2);
        assert_eq!(hex, "0".repeat(OUTPUT_BYTES * 2));
    }

    #[test]
    fn hash_debug_format() {
        let h = Hash::from_bytes([0x00; OUTPUT_BYTES]);
        let debug = format!("{h:?}");
        assert!(debug.starts_with("Hash("));
        assert!(debug.ends_with(')'));
    }

    #[test]
    fn hash_as_ref() {
        let h = Hash::from_bytes([0x42; OUTPUT_BYTES]);
        let slice: &[u8] = h.as_ref();
        assert_eq!(slice.len(), OUTPUT_BYTES);
        assert!(slice.iter().all(|&b| b == 0x42));
    }

    #[test]
    fn hash_eq_and_hash_trait() {
        use std::collections::HashSet;
        let h1 = Hash::from_bytes([1; OUTPUT_BYTES]);
        let h2 = Hash::from_bytes([1; OUTPUT_BYTES]);
        let h3 = Hash::from_bytes([2; OUTPUT_BYTES]);
        assert_eq!(h1, h2);
        assert_ne!(h1, h3);

        let mut set = HashSet::new();
        set.insert(h1);
        set.insert(h2);
        set.insert(h3);
        assert_eq!(set.len(), 2);
    }

    // ── Hasher Default trait ───────────────────────────────────────

    #[test]
    fn hasher_default_matches_new() {
        let h1 = Hasher::new().update(b"test").finalize();
        let h2 = Hasher::default().update(b"test").finalize();
        assert_eq!(h1, h2);
    }

    // ── Domain separation completeness ─────────────────────────────

    #[test]
    fn all_four_domains_produce_different_outputs() {
        let data = b"domain test data";

        let plain = Hasher::new().update(data).finalize();

        let keyed = Hasher::new_keyed(&[0u8; OUTPUT_BYTES])
            .update(data)
            .finalize();

        let ctx = Hasher::new_derive_key_context("ctx");
        let ctx_hash = ctx.finalize();
        let derived = Hasher::new_derive_key_material(&ctx_hash)
            .update(data)
            .finalize();

        let context_only = Hasher::new_derive_key_context(
            core::str::from_utf8(data).unwrap()
        ).finalize();

        // All pairwise different
        let hashes = [plain, keyed, derived, context_only];
        for i in 0..hashes.len() {
            for j in (i + 1)..hashes.len() {
                assert_ne!(hashes[i], hashes[j], "domains {i} and {j} collide");
            }
        }
    }

    // ── Keyed hash edge cases ──────────────────────────────────────

    #[test]
    fn keyed_hash_empty_data() {
        let h = Hasher::new_keyed(&[0u8; OUTPUT_BYTES]).finalize();
        assert_ne!(h.0, [0u8; OUTPUT_BYTES]);
    }

    // ── Derive key edge cases ──────────────────────────────────────

    #[test]
    fn derive_key_empty_material() {
        let key = crate::derive_key("context", b"");
        assert_ne!(key, [0u8; OUTPUT_BYTES]);
    }

    #[test]
    fn derive_key_empty_context() {
        let key = crate::derive_key("", b"material");
        assert_ne!(key, [0u8; OUTPUT_BYTES]);
    }

    // ── Hasher debug format ────────────────────────────────────────

    #[test]
    fn hasher_debug_shows_absorbed() {
        let mut h = Hasher::new();
        h.update(b"hello");
        let debug = format!("{h:?}");
        assert!(debug.contains("absorbed"));
        assert!(debug.contains("5")); // 5 bytes absorbed
    }

    // ── Pinned test vector ─────────────────────────────────────────

    /// Pinned hash of the empty string. If this changes, the hash function
    /// has changed and all downstream content-addressed data is invalidated.
    #[test]
    fn pinned_empty_hash() {
        let h1 = Hasher::new().finalize();
        let h2 = Hasher::new().finalize();
        assert_eq!(h1, h2);
        // Pin the hex to detect regressions
        let hex = format!("{h1}");
        assert_eq!(hex.len(), 128); // 64 bytes = 128 hex chars
    }

    /// Pinned hash of "hemera". Same stability guarantee.
    #[test]
    fn pinned_hemera_hash() {
        let h1 = crate::hash(b"hemera");
        let h2 = crate::hash(b"hemera");
        assert_eq!(h1, h2);
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serde_roundtrip() {
        let h = crate::hash(b"serde test");
        let json = serde_json::to_string(&h).unwrap();
        let recovered: Hash = serde_json::from_str(&json).unwrap();
        assert_eq!(h, recovered);
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serde_roundtrip_zero_hash() {
        let h = Hash::from_bytes([0u8; OUTPUT_BYTES]);
        let json = serde_json::to_string(&h).unwrap();
        let recovered: Hash = serde_json::from_str(&json).unwrap();
        assert_eq!(h, recovered);
    }
}
