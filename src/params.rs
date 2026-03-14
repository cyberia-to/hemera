//! Hemera — Poseidon2 parameter set over the Goldilocks field.
//!
//! Single source of truth for every constant in the protocol.
//! The WGSL shader (`gpu/poseidon2.wgsl`) duplicates a subset of
//! these values because WGSL cannot import Rust; keep them in sync.
//!
//! ```text
//! ┌──────────────────────────────────────────────────────────┐
//! │  HEMERA — Complete Specification                         │
//! │                                                          │
//! │  Field:           p = 2⁶⁴ − 2³² + 1 (Goldilocks)       │
//! │  S-box:           d = 7  (x → x⁷, minimum for field)    │
//! │  State width:     t = 16                      = 2⁴       │
//! │  Full rounds:     R_F = 8  (4 + 4)            = 2³       │
//! │  Partial rounds:  R_P = 64                    = 2⁶       │
//! │  Rate:            r = 8  elements (56 bytes)  = 2³       │
//! │  Capacity:        c = 8  elements (64 bytes)  = 2³       │
//! │  Output:          8  elements (64 bytes)      = 2³       │
//! │                                                          │
//! │  Full round constants:    8 × 16 = 128        = 2⁷       │
//! │  Partial round constants: 64                  = 2⁶       │
//! │  Total constants:         192                 = 3 × 2⁶   │
//! │  Total rounds:            72                  = 9 × 2³   │
//! │                                                          │
//! │  Classical collision resistance:  256 bits     = 2⁸       │
//! │  Quantum collision resistance:   170 bits                │
//! │  Algebraic degree:               2¹⁸⁰                    │
//! │                                                          │
//! │  Every parameter that appears in code is a power of 2.   │
//! └──────────────────────────────────────────────────────────┘
//! ```

use std::sync::LazyLock;

use p3_goldilocks::{Goldilocks, Poseidon2Goldilocks};
use p3_symmetric::Permutation;

use crate::constants::ROUND_CONSTANTS;

// ── Permutation parameters ──────────────────────────────────────────

/// Width of the Poseidon2 state (number of Goldilocks field elements).
pub const WIDTH: usize = 16;

/// Number of full (external) rounds — 4 initial + 4 final.
pub const ROUNDS_F: usize = 8;

/// Number of partial (internal) rounds.
pub const ROUNDS_P: usize = 64;

/// S-box degree (x → x^d).
pub const SBOX_DEGREE: usize = 7;

// ── Sponge parameters ───────────────────────────────────────────────

/// Number of rate elements in the sponge.
pub const RATE: usize = 8;

/// Number of capacity elements in the sponge.
pub const CAPACITY: usize = WIDTH - RATE; // 8

// ── Encoding parameters ─────────────────────────────────────────────

/// Bytes per field element when encoding arbitrary input data.
///
/// We use 7 bytes per element because 2^56 − 1 < p (Goldilocks prime),
/// so any 7-byte value fits without reduction.
pub const INPUT_BYTES_PER_ELEMENT: usize = 7;

/// Bytes per field element when encoding hash output.
///
/// For output we use the full canonical u64 representation (8 bytes),
/// since output elements are already valid field elements.
pub const OUTPUT_BYTES_PER_ELEMENT: usize = 8;

// ── Derived constants ───────────────────────────────────────────────

/// Number of input bytes that fill one rate block (8 elements × 7 bytes).
pub const RATE_BYTES: usize = RATE * INPUT_BYTES_PER_ELEMENT; // 56

/// Number of output elements extracted per squeeze (= rate).
pub const OUTPUT_ELEMENTS: usize = RATE; // 8

/// Number of output bytes per squeeze (8 elements × 8 bytes).
pub const OUTPUT_BYTES: usize = OUTPUT_ELEMENTS * OUTPUT_BYTES_PER_ELEMENT; // 64

// ── Tree parameters ─────────────────────────────────────────────────

/// Canonical chunk size for content tree construction (4 KB).
///
/// Content is split into fixed 4 KB chunks. Each chunk is hashed via
/// `chunk_cv`. The last chunk may be shorter. See spec §4.6.1.
pub const CHUNK_SIZE: usize = 4096;

// ── Security properties (informational) ─────────────────────────────

/// Classical collision resistance in bits.
pub const COLLISION_BITS: usize = 256;

// ── Permutation singleton ───────────────────────────────────────────

/// Global singleton Poseidon2 permutation instance.
///
/// Round constants are loaded from the pre-computed static array in
/// `constants.rs` (originally derived via self-bootstrapping from the
/// genesis seed "cyber").
static POSEIDON2: LazyLock<Poseidon2Goldilocks<WIDTH>> = LazyLock::new(|| {
    let mut rng = ConstantRng { pos: 0 };
    Poseidon2Goldilocks::new_from_rng(ROUNDS_F, ROUNDS_P, &mut rng)
});

/// RNG that replays the pre-computed round constants.
struct ConstantRng {
    pos: usize,
}

impl rand::RngCore for ConstantRng {
    fn next_u32(&mut self) -> u32 {
        self.next_u64() as u32
    }

    fn next_u64(&mut self) -> u64 {
        let val = ROUND_CONSTANTS[self.pos];
        self.pos += 1;
        val
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        let mut written = 0;
        while written < dest.len() {
            let val = self.next_u64();
            let bytes = val.to_le_bytes();
            let remaining = dest.len() - written;
            let n = remaining.min(8);
            dest[written..written + n].copy_from_slice(&bytes[..n]);
            written += n;
        }
    }
}

/// Apply the Poseidon2 permutation in-place.
pub(crate) fn permute(state: &mut [Goldilocks; WIDTH]) {
    POSEIDON2.permute_mut(state);
}

#[cfg(test)]
mod tests {
    use super::*;
    use p3_goldilocks::Goldilocks;

    #[test]
    fn permutation_is_deterministic() {
        let mut s1 = [Goldilocks::new(0); WIDTH];
        let mut s2 = [Goldilocks::new(0); WIDTH];
        permute(&mut s1);
        permute(&mut s2);
        assert_eq!(s1, s2);
    }

    #[test]
    fn permutation_changes_state() {
        let mut state = [Goldilocks::new(0); WIDTH];
        let original = state;
        permute(&mut state);
        assert_ne!(state, original);
    }

    #[test]
    fn different_inputs_different_outputs() {
        let mut s1 = [Goldilocks::new(0); WIDTH];
        let mut s2 = [Goldilocks::new(0); WIDTH];
        s2[0] = Goldilocks::new(1);
        permute(&mut s1);
        permute(&mut s2);
        assert_ne!(s1, s2);
    }

    #[test]
    fn sponge_geometry() {
        assert_eq!(WIDTH, RATE + CAPACITY);
        assert_eq!(RATE_BYTES, 56);
        assert_eq!(OUTPUT_BYTES, 64);
        assert_eq!(OUTPUT_ELEMENTS, 8);
    }
}
