// ---
// tags: hemera, rust
// crystal-type: source
// crystal-domain: comp
// ---
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
//! │  Full-round S-box: d = 7  (x → x⁷)                     │
//! │  Partial S-box:   x⁻¹    (field inversion)              │
//! │  State width:     t = 16                      = 2⁴       │
//! │  Full rounds:     R_F = 8  (4 + 4)            = 2³       │
//! │  Partial rounds:  R_P = 16                    = 2⁴       │
//! │  Rate:            r = 8  elements              = 2³       │
//! │  Input rate:      56 bytes/block (7 B/element) = 7 × 2³   │
//! │  Capacity:        c = 8  elements (64 bytes)   = 2³       │
//! │  Output:          4  elements (32 bytes)       = 2²       │
//! │                                                          │
//! │  Full round constants:    8 × 16 = 128        = 2⁷       │
//! │  Partial round constants: 16                  = 2⁴       │
//! │  Total constants:         144                 = 9 × 2⁴   │
//! │  Total rounds:            24                  = 3 × 2³   │
//! │                                                          │
//! │  Classical collision resistance:  256 bits     = 2⁸       │
//! │  Quantum collision resistance:   170 bits                │
//! │  Algebraic degree:               2¹⁰⁴⁶                   │
//! │                                                          │
//! │  Every parameter that appears in code is a power of 2.   │
//! └──────────────────────────────────────────────────────────┘
//! ```

use crate::field::Goldilocks;

// ── Permutation parameters ──────────────────────────────────────────

/// Width of the Poseidon2 state (number of Goldilocks field elements).
pub const WIDTH: usize = 16;

/// Number of full (external) rounds — 4 initial + 4 final.
pub const ROUNDS_F: usize = 8;

/// Number of partial (internal) rounds.
pub const ROUNDS_P: usize = 16;

/// Full-round S-box degree (x → x^d).
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

/// Number of output elements extracted per squeeze (4 elements = 32 bytes).
pub const OUTPUT_ELEMENTS: usize = 4;

/// Number of output bytes per squeeze (4 elements × 8 bytes).
pub const OUTPUT_BYTES: usize = OUTPUT_ELEMENTS * OUTPUT_BYTES_PER_ELEMENT; // 32

// ── Tree parameters ─────────────────────────────────────────────────

/// Canonical chunk size for content tree construction (4 KB).
///
/// Content is split into fixed 4 KB chunks. Each chunk is hashed via
/// `hash_leaf`. The last chunk may be shorter. See spec §4.6.1.
pub const CHUNK_SIZE: usize = 4096;

/// Maximum tree depth (sufficient for 2^64 chunks).
pub const MAX_TREE_DEPTH: usize = 64;

// ── Security properties (informational) ─────────────────────────────

/// Classical collision resistance in bits.
pub const COLLISION_BITS: usize = 256;

// ── Permutation entry point ─────────────────────────────────────────

/// Apply the Poseidon2 permutation in-place.
#[inline]
pub(crate) fn permute(state: &mut [Goldilocks; WIDTH]) {
    crate::permutation::permute(state);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permutation_is_deterministic() {
        let mut s1 = [Goldilocks::ZERO; WIDTH];
        let mut s2 = [Goldilocks::ZERO; WIDTH];
        permute(&mut s1);
        permute(&mut s2);
        assert_eq!(s1, s2);
    }

    #[test]
    fn permutation_changes_state() {
        let mut state = [Goldilocks::ZERO; WIDTH];
        let original = state;
        permute(&mut state);
        assert_ne!(state, original);
    }

    #[test]
    fn different_inputs_different_outputs() {
        let mut s1 = [Goldilocks::ZERO; WIDTH];
        let mut s2 = [Goldilocks::ZERO; WIDTH];
        s2[0] = Goldilocks::new(1);
        permute(&mut s1);
        permute(&mut s2);
        assert_ne!(s1, s2);
    }

    #[test]
    fn sponge_geometry() {
        assert_eq!(WIDTH, RATE + CAPACITY);
        assert_eq!(RATE_BYTES, 56);
        assert_eq!(OUTPUT_BYTES, 32);
        assert_eq!(OUTPUT_ELEMENTS, 4);
    }
}
