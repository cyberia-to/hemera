// ---
// tags: hemera, rust
// crystal-type: source
// crystal-domain: comp
// ---
//! Per-round trace API for Poseidon2 permutation proof generation.
//!
//! [`RoundVisitor`] receives one callback per round — 60 per permutation
//! (12 full + 48 partial) — with the state and S-box witnesses needed to
//! build degree-3 AIR constraints. Every round uses the x⁷ S-box, so a
//! full round carries 16 witnesses and a partial round one.
//!
//! ## Row count
//!
//! A plain sponge hashing N absorb blocks emits (N + 1) × 60 rows via
//! [`Hasher::update_traced`] / [`Hasher::finalize_traced`].

use crate::field::Goldilocks;

/// Witness of one x⁷ S-box: `[x², x³]` where `x` is the S-box input
/// (after the round constant is added).
///
/// Degree-3 AIR constraints:
/// ```text
/// x · x        = x²    (degree 2)
/// x · x²       = x³    (degree 2)
/// x³ · x³ · x  = x^7   (degree 3)
/// ```
pub type SboxWitnesses = [Goldilocks; 2];

/// Per-element S-box witnesses for a full round: `witnesses[i]` belongs to
/// `state[i]`.
pub type FullRoundWitnesses = [SboxWitnesses; 16];

/// Called once per round during a traced Poseidon2 permutation.
///
/// The `state` argument is always the complete state *after* the round
/// (add_rc + S-box + linear layer). Witness values supply S-box intermediates
/// that are not recoverable from the post-round state alone.
pub trait RoundVisitor {
    /// After a full round: add_rc on all 16 elements + x⁷ S-box + M_E.
    ///
    /// `index` ∈ 0..12 — rounds 0–5 are the initial group; 6–11 are terminal.
    fn full_round(
        &mut self,
        index: u8,
        state: &[Goldilocks; 16],
        witnesses: &FullRoundWitnesses,
    );

    /// After a partial round: add_rc on state[0] + x⁷ S-box on state[0] + M_I.
    ///
    /// `index` ∈ 0..48. `witness` is `[x², x³]` of the S-box input `x`
    /// (state[0] after its round constant).
    fn partial_round(&mut self, index: u8, state: &[Goldilocks; 16], witness: SboxWitnesses);
}
