use std::fmt;
use std::io;

use p3_goldilocks::Goldilocks;

use crate::encoding::{bytes_to_rate_block, hash_to_bytes};
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
    pub fn to_hex(&self) -> String {
        let mut s = String::with_capacity(OUTPUT_BYTES * 2);
        for byte in &self.0 {
            use fmt::Write;
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
#[derive(Clone)]
pub struct Hasher {
    state: [Goldilocks; WIDTH],
    buf: Vec<u8>,
    absorbed: u64,
}

impl Hasher {
    /// Create a new hasher in plain hash mode.
    pub fn new() -> Self {
        let mut state = [Goldilocks::new(0); WIDTH];
        state[CAPACITY_START + 3] = Goldilocks::new(DOMAIN_HASH);
        Self {
            state,
            buf: Vec::new(),
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
            buf: Vec::new(),
            absorbed: 0,
        };
        hasher.update(key.as_slice());
        hasher
    }

    /// Create a new hasher in derive-key mode.
    ///
    /// First hashes the context string to produce a context key, then
    /// sets up a second hasher seeded with that key for absorbing key material.
    pub(crate) fn new_derive_key_context(context: &str) -> Self {
        let mut state = [Goldilocks::new(0); WIDTH];
        state[CAPACITY_START + 3] = Goldilocks::new(DOMAIN_DERIVE_KEY_CONTEXT);
        let mut hasher = Self {
            state,
            buf: Vec::new(),
            absorbed: 0,
        };
        hasher.update(context.as_bytes());
        hasher
    }

    /// Create a derive-key hasher for the material phase, seeded by a context hash.
    pub(crate) fn new_derive_key_material(context_hash: &Hash) -> Self {
        let mut state = [Goldilocks::new(0); WIDTH];
        state[CAPACITY_START + 3] = Goldilocks::new(DOMAIN_DERIVE_KEY_MATERIAL);

        // Seed the rate portion with the context hash (4 elements = 32 bytes).
        for (i, chunk) in context_hash.0.chunks(OUTPUT_BYTES_PER_ELEMENT).enumerate() {
            let val = u64::from_le_bytes(chunk.try_into().unwrap());
            state[i] = Goldilocks::new(val);
        }
        params::permute(&mut state);

        Self {
            state,
            buf: Vec::new(),
            absorbed: 0,
        }
    }

    /// Absorb input data into the sponge.
    pub fn update(&mut self, data: &[u8]) -> &mut Self {
        self.buf.extend_from_slice(data);
        self.absorbed += data.len() as u64;

        // Process complete rate blocks.
        while self.buf.len() >= RATE_BYTES {
            let block_bytes: Vec<u8> = self.buf.drain(..RATE_BYTES).collect();
            let mut rate_block = [Goldilocks::new(0); RATE];
            bytes_to_rate_block(&block_bytes, &mut rate_block);
            self.absorb_block(&rate_block);
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
    fn finalize_state(&self) -> [Goldilocks; WIDTH] {
        let mut state = self.state;
        let mut padded = self.buf.clone();

        // Append padding marker (Hemera: 0x01).
        padded.push(0x01);

        // Pad to full rate block.
        padded.resize(RATE_BYTES, 0x00);

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
            .field("buffered", &self.buf.len())
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

impl io::Read for OutputReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
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
                h.update(std::slice::from_ref(b));
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
}
