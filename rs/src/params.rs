// ---
// tags: hemera, rust
// crystal-type: source
// crystal-domain: comp
// ---
//! Hemera — Poseidon2 parameter set over the Goldilocks field (profile 0.4).
//!
//! Single source of truth for every constant in the protocol.
//! The WGSL shaders (`wgsl/src/shaders/params.wgsl`) duplicate a subset of
//! these values because WGSL cannot import Rust; keep them in sync.
//!
//! ```text
//! ┌──────────────────────────────────────────────────────────┐
//! │  HEMERA 0.4 — Complete Specification                     │
//! │                                                          │
//! │  Field:           p = 2⁶⁴ − 2³² + 1 (Goldilocks)         │
//! │  S-box:           x⁷ in every round (full and partial)   │
//! │  State width:     t = 16                                 │
//! │  Full rounds:     R_F = 12  (6 + 6)                      │
//! │  Partial rounds:  R_P = 48                               │
//! │  External layer:  circ(2·M4, M4, M4, M4), M4 Poseidon2   │
//! │  Internal layer:  1 + diag(d), d Poseidon2 reference     │
//! │  Rate:            r = 8  elements                        │
//! │  Input rate:      56 bytes/block (7 B/element)           │
//! │  Capacity:        c = 8  elements (64 bytes)             │
//! │  Output:          4  elements (32 bytes)                 │
//! │                                                          │
//! │  Full round constants:    12 × 16 = 192                  │
//! │  Partial round constants: 48                             │
//! │  Total constants:         240                            │
//! │  Total rounds:            60                             │
//! │                                                          │
//! │  Collision (32-byte digest): 2¹²⁸ classical, 2⁸⁵·³ BHT   │
//! │  Preimage  (32-byte digest): 2²⁵⁶ classical, 2¹²⁸ Grover │
//! │  Algebraic margin: measured by the campaign, not claimed │
//! └──────────────────────────────────────────────────────────┘
//! ```

use crate::field::Goldilocks;

// ── Permutation parameters ──────────────────────────────────────────

/// Width of the Poseidon2 state (number of Goldilocks field elements).
pub const WIDTH: usize = 16;

/// Number of full (external) rounds — 6 initial + 6 final.
pub const ROUNDS_F: usize = 12;

/// Number of partial (internal) rounds.
pub const ROUNDS_P: usize = 48;

/// Total number of rounds per permutation (R_F + R_P).
pub const ROUNDS_TOTAL: usize = ROUNDS_F + ROUNDS_P; // 60

/// S-box degree (x → x^d), every round: the least d with gcd(d, p − 1) = 1.
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

/// Classical collision resistance of the 32-byte digest in bits
/// (birthday bound on p⁴ ≈ 2²⁵⁶ outputs). Quantum (BHT): ≈ 85.3 bits.
pub const COLLISION_BITS: usize = 128;

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
