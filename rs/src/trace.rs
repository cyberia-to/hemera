// ---
// tags: hemera, rust
// crystal-type: source
// crystal-domain: comp
// ---
//! Per-round trace API for Poseidon2 permutation proof generation.
//!
//! [`RoundVisitor`] receives one callback per round — 24 per permutation
//! (8 full + 16 partial) — with the state and S-box witnesses needed to
//! build degree-3 AIR constraints.
//!
//! ## Row count
//!
//! A plain sponge hashing N absorb blocks emits (N + 1) × 24 rows via
//! [`Hasher::update_traced`] / [`Hasher::finalize_traced`].

use crate::field::Goldilocks;

/// Per-element S-box witnesses for a full round.
///
/// `witnesses[i] = [x², x³]` where `x = state[i]` after adding the round
/// constant, before the x^7 S-box.
///
/// Degree-3 AIR constraints:
/// ```text
/// x · x        = x²    (degree 2)
/// x · x²       = x³    (degree 2)
/// x³ · x³ · x  = x^7   (degree 3)
/// ```
pub type FullRoundWitnesses = [[Goldilocks; 2]; 16];

/// Called once per round during a traced Poseidon2 permutation.
///
/// The `state` argument is always the complete state *after* the round
/// (add_rc + S-box + linear layer). Witness values supply S-box intermediates
/// that are not recoverable from the post-round state alone.
pub trait RoundVisitor {
    /// After a full round: add_rc on all 16 elements + x^7 S-box + MDS.
    ///
    /// `index` ∈ 0..8 — rounds 0–3 are the initial group; 4–7 are terminal.
    /// `witnesses[i] = [state_pre_sbox[i]², state_pre_sbox[i]³]`.
    fn full_round(
        &mut self,
        index: u8,
        state: &[Goldilocks; 16],
        witnesses: &FullRoundWitnesses,
    );

    /// After a partial round: add_rc on state[0] + x^(−1) S-box + matmul_internal.
    ///
    /// `index` ∈ 0..16.
    /// `sbox_out` is `state[0]^(−1)` captured *after* inversion and *before*
    /// matmul_internal. Constraint: `(state[0] post-add_rc) · sbox_out = 1`.
    fn partial_round(&mut self, index: u8, state: &[Goldilocks; 16], sbox_out: Goldilocks);
}
