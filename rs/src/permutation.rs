// ---
// tags: hemera, rust
// crystal-type: source
// crystal-domain: comp
// ---
//! Poseidon2 permutation for Goldilocks t=16.
//!
//! Hemera parameters: R_F=8 (4+4), R_P=16, full-round S-box=x^7, partial S-box=x^(-1).

use crate::constants::ROUND_CONSTANTS;
use crate::field::{Goldilocks, matmul_internal, mds_light_permutation};
use crate::trace::{FullRoundWitnesses, RoundVisitor};

/// Number of external (full) round constants: R_F * WIDTH = 8 * 16 = 128.
const NUM_EXTERNAL: usize = 128;

/// Apply the Poseidon2 permutation in-place using the standard Hemera constants.
///
/// Structure: initial MDS → 4 full rounds → 16 partial rounds → 4 full rounds.
pub fn permute(state: &mut [Goldilocks; 16]) {
    permute_with_constants(state, &ROUND_CONSTANTS);
}

/// Apply the Poseidon2 permutation with caller-supplied round constants.
///
/// Used by `bootstrap.rs` to run Hemera₀ (all-zero constants).
pub fn permute_with_constants(state: &mut [Goldilocks; 16], constants: &[Goldilocks]) {
    let (external, internal) = constants.split_at(NUM_EXTERNAL);

    // Split external constants into initial (first 4 rounds) and terminal (last 4 rounds).
    let (initial_rc, terminal_rc) = external.split_at(NUM_EXTERNAL / 2);

    // ── Initial external rounds ─────────────────────────────────
    // One MDS multiplication before the first round.
    mds_light_permutation(state);

    // 4 initial full rounds: add_rc + sbox_all + MDS
    for round in 0..4 {
        let rc = &initial_rc[round * 16..(round + 1) * 16];
        for i in 0..16 {
            state[i] += rc[i];
            state[i] = state[i].pow7();
        }
        mds_light_permutation(state);
    }

    // ── Internal (partial) rounds ───────────────────────────────
    // 16 partial rounds: add_rc to state[0] + sbox state[0] (field inversion) + diffusion
    for round in 0..16 {
        state[0] += internal[round];
        state[0] = state[0].inv();
        matmul_internal(state);
    }

    // ── Terminal external rounds ─────────────────────────────────
    // 4 terminal full rounds: add_rc + sbox_all + MDS
    for round in 0..4 {
        let rc = &terminal_rc[round * 16..(round + 1) * 16];
        for i in 0..16 {
            state[i] += rc[i];
            state[i] = state[i].pow7();
        }
        mds_light_permutation(state);
    }
}

/// Apply the Poseidon2 permutation in-place, calling `visitor` once per round.
///
/// Emits 24 callbacks total in order: full_round 0–3, partial_round 0–15, full_round 4–7.
/// The initial MDS step is linear and does not emit a callback.
pub fn permute_traced<V: RoundVisitor>(state: &mut [Goldilocks; 16], visitor: &mut V) {
    let (external, internal) = ROUND_CONSTANTS.split_at(NUM_EXTERNAL);
    let (initial_rc, terminal_rc) = external.split_at(NUM_EXTERNAL / 2);

    mds_light_permutation(state);

    for round in 0..4u8 {
        let rc = &initial_rc[round as usize * 16..(round as usize + 1) * 16];
        let witnesses = full_round_step(state, rc);
        visitor.full_round(round, state, &witnesses);
    }

    for round in 0..16u8 {
        state[0] += internal[round as usize];
        state[0] = state[0].inv();
        let sbox_out = state[0];
        matmul_internal(state);
        visitor.partial_round(round, state, sbox_out);
    }

    for round in 0..4u8 {
        let rc = &terminal_rc[round as usize * 16..(round as usize + 1) * 16];
        let witnesses = full_round_step(state, rc);
        visitor.full_round(round + 4, state, &witnesses);
    }
}

/// Add round constants, apply x^7 S-box to all 16 elements, apply MDS.
/// Returns per-element witnesses `[x², x³]` for the pre-S-box state.
#[inline]
fn full_round_step(state: &mut [Goldilocks; 16], rc: &[Goldilocks]) -> FullRoundWitnesses {
    let mut witnesses = [[Goldilocks::ZERO; 2]; 16];
    for i in 0..16 {
        state[i] += rc[i];
        let x2 = state[i] * state[i];
        let x3 = x2 * state[i];
        let x4 = x2 * x2;
        witnesses[i] = [x2, x3];
        state[i] = x3 * x4; // x^7
    }
    mds_light_permutation(state);
    witnesses
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trace::{FullRoundWitnesses, RoundVisitor};

    struct RoundCounter {
        full: u8,
        partial: u8,
    }

    impl RoundVisitor for RoundCounter {
        fn full_round(&mut self, _: u8, _: &[Goldilocks; 16], _: &FullRoundWitnesses) {
            self.full += 1;
        }
        fn partial_round(&mut self, _: u8, _: &[Goldilocks; 16], _: Goldilocks) {
            self.partial += 1;
        }
    }

    #[test]
    fn traced_matches_plain() {
        let mut state_plain = [Goldilocks::new(42); 16];
        let mut state_traced = state_plain;
        let mut counter = RoundCounter { full: 0, partial: 0 };

        permute(&mut state_plain);
        permute_traced(&mut state_traced, &mut counter);

        assert_eq!(state_plain, state_traced);
        assert_eq!(counter.full, 8);
        assert_eq!(counter.partial, 16);
    }

    #[test]
    fn trace_round_indices_are_ordered() {
        struct IndexRecorder {
            full_indices: [u8; 8],
            full_count: usize,
            partial_indices: [u8; 16],
            partial_count: usize,
        }
        impl RoundVisitor for IndexRecorder {
            fn full_round(&mut self, index: u8, _: &[Goldilocks; 16], _: &FullRoundWitnesses) {
                self.full_indices[self.full_count] = index;
                self.full_count += 1;
            }
            fn partial_round(&mut self, index: u8, _: &[Goldilocks; 16], _: Goldilocks) {
                self.partial_indices[self.partial_count] = index;
                self.partial_count += 1;
            }
        }

        let mut state = [Goldilocks::ZERO; 16];
        let mut rec = IndexRecorder {
            full_indices: [0; 8],
            full_count: 0,
            partial_indices: [0; 16],
            partial_count: 0,
        };
        permute_traced(&mut state, &mut rec);

        for i in 0..8u8 {
            assert_eq!(rec.full_indices[i as usize], i, "full round index {i}");
        }
        for i in 0..16u8 {
            assert_eq!(rec.partial_indices[i as usize], i, "partial round index {i}");
        }
    }

    #[test]
    fn full_round_witnesses_correct() {
        struct WitnessChecker;
        impl RoundVisitor for WitnessChecker {
            fn full_round(&mut self, _: u8, _: &[Goldilocks; 16], _: &FullRoundWitnesses) {}
            fn partial_round(&mut self, _: u8, _: &[Goldilocks; 16], _: Goldilocks) {}
        }
        // Just confirm it compiles and runs without panic.
        let mut state = [Goldilocks::new(7); 16];
        permute_traced(&mut state, &mut WitnessChecker);
    }

    #[test]
    fn permutation_is_deterministic() {
        let mut s1 = [Goldilocks::ZERO; 16];
        let mut s2 = [Goldilocks::ZERO; 16];
        permute(&mut s1);
        permute(&mut s2);
        assert_eq!(s1, s2);
    }

    #[test]
    fn permutation_changes_state() {
        let mut state = [Goldilocks::ZERO; 16];
        let original = state;
        permute(&mut state);
        assert_ne!(state, original);
    }

    #[test]
    fn different_inputs_different_outputs() {
        let mut s1 = [Goldilocks::ZERO; 16];
        let mut s2 = [Goldilocks::ZERO; 16];
        s2[0] = Goldilocks::new(1);
        permute(&mut s1);
        permute(&mut s2);
        assert_ne!(s1, s2);
    }
}
