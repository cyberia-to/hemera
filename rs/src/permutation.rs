// ---
// tags: hemera, rust
// crystal-type: source
// crystal-domain: comp
// ---
//! Poseidon2 permutation for Goldilocks t=16.
//!
//! Hemera 0.4: R_F = 12 (6 + 6), R_P = 48, S-box x⁷ in every round.

use crate::backend::{Ct, Fast};
use crate::constants::{NUM_CONSTANTS, NUM_EXTERNAL, ROUND_CONSTANTS};
use crate::field::{Goldilocks, matmul_internal, mds_light_permutation};
use crate::kernel::{Lanes, permute_lanes};
use crate::params::{ROUNDS_F, ROUNDS_P};
pub use crate::profile::{HEMERA, Profile};
use crate::trace::{RoundVisitor, SboxWitnesses};

/// Full rounds on each side of the partial rounds.
const RF_HALF: usize = ROUNDS_F / 2; // 6

/// Lanes per batched kernel call (see `permute_batch`).
pub const BATCH_LANES: usize = 16;

/// Apply the Poseidon2 permutation in-place using the standard Hemera constants.
///
/// Structure: initial M_E → 6 full rounds → 48 partial rounds → 6 full rounds.
/// Output elements are canonical.
pub fn permute(state: &mut [Goldilocks; 16]) {
    permute_profile(state, &HEMERA);
}

/// Apply the permutation of `profile` in-place (single state).
pub fn permute_profile(state: &mut [Goldilocks; 16], profile: &Profile<'_>) {
    let mut s: Lanes<1> = core::array::from_fn(|i| [state[i].raw()]);
    permute_lanes::<Fast, 1>(&mut s, profile);
    for (dst, src) in state.iter_mut().zip(s.iter()) {
        *dst = Goldilocks::new(src[0]);
    }
}

/// The permutation in constant time: no branch and no memory access
/// depends on the state. Bit-identical to [`permute`] (same limbs).
///
/// Every hemera entry point that can see secret input runs this kernel
/// internally — `keyed_hash`, `derive_key`, `hash_secret` and the
/// `Hasher`s they build (see `specs/api.md` § constant time). Call it
/// directly only to build another secret-input construction.
#[inline(never)]
pub fn permute_ct(state: &mut [Goldilocks; 16]) {
    let mut s: Lanes<1> = core::array::from_fn(|i| [state[i].raw()]);
    permute_lanes::<Ct, 1>(&mut s, &HEMERA);
    for (dst, src) in state.iter_mut().zip(s.iter()) {
        *dst = Goldilocks::new(src[0]);
    }
}

/// Apply the permutation to every state in `states` (independent inputs).
///
/// Equal, state by state, to calling [`permute`] on each; runs
/// [`BATCH_LANES`] permutations interleaved per kernel call (a shorter
/// tail in groups of 8, 4, 2), so independent multiplications of
/// different states fill the pipeline beside each state's serial chain.
pub fn permute_batch(states: &mut [[Goldilocks; 16]]) {
    permute_batch_profile(states, &HEMERA);
}

/// [`permute_batch`] for any profile.
pub fn permute_batch_profile(states: &mut [[Goldilocks; 16]], profile: &Profile<'_>) {
    // Full groups of BATCH_LANES, then the remainder through narrower
    // kernels (8, 4, 2 lanes) so a short tail still runs interleaved.
    let rest = run_groups::<BATCH_LANES>(states, profile);
    let rest = run_groups::<8>(rest, profile);
    let rest = run_groups::<4>(rest, profile);
    let rest = run_groups::<2>(rest, profile);
    for st in rest {
        permute_profile(st, profile);
    }
}

/// Permute `states` in groups of `L` lanes; return the ungrouped tail.
fn run_groups<'s, const L: usize>(
    states: &'s mut [[Goldilocks; 16]],
    profile: &Profile<'_>,
) -> &'s mut [[Goldilocks; 16]] {
    let mut chunks = states.chunks_exact_mut(L);
    for chunk in &mut chunks {
        let mut s: Lanes<L> = core::array::from_fn(|i| core::array::from_fn(|l| chunk[l][i].raw()));
        permute_lanes::<Fast, L>(&mut s, profile);
        for (l, st) in chunk.iter_mut().enumerate() {
            for (i, e) in st.iter_mut().enumerate() {
                *e = Goldilocks::new(s[i][l]);
            }
        }
    }
    chunks.into_remainder()
}

/// Apply the Poseidon2 permutation with caller-supplied round constants.
///
/// Used by `bootstrap.rs` to run Hemera₀ (all-zero constants).
/// `constants` = 192 external then 48 internal; any representatives.
pub fn permute_with_constants(state: &mut [Goldilocks; 16], constants: &[Goldilocks]) {
    let mut raw = [0u64; NUM_CONSTANTS];
    assert_eq!(
        constants.len(),
        NUM_CONSTANTS,
        "permute_with_constants: 240 constants"
    );
    for (r, c) in raw.iter_mut().zip(constants) {
        *r = c.as_canonical_u64();
    }
    let (external, internal) = raw.split_at(NUM_EXTERNAL);
    let profile = Profile::new(HEMERA.rf_half, HEMERA.rp, external, internal, HEMERA.diag);
    permute_profile(state, &profile);
}

/// Execute one round of the permutation in-place.
///
/// `round` is 0-indexed over all 60 rounds:
/// - 0..6:   initial full rounds (after the initial M_E, which is applied separately)
/// - 6..54:  partial rounds
/// - 54..60: terminal full rounds
///
/// The initial M_E step must be applied once before `permute_one_round(state, 0)`.
/// See `StepSponge::absorb`, which handles this.
pub(crate) fn permute_one_round(state: &mut [Goldilocks; 16], round: usize) {
    let (external, internal) = ROUND_CONSTANTS.split_at(NUM_EXTERNAL);
    if round < RF_HALF {
        full_round_step(state, &external[round * 16..(round + 1) * 16]);
    } else if round < RF_HALF + ROUNDS_P {
        partial_round_step(state, internal[round - RF_HALF]);
    } else {
        let r = round - ROUNDS_P; // external row RF_HALF..ROUNDS_F
        full_round_step(state, &external[r * 16..(r + 1) * 16]);
        if round == ROUNDS_F + ROUNDS_P - 1 {
            canonicalize(state);
        }
    }
}

/// Reduce every limb to `[0, p)` — the representation `permute` returns,
/// so the stepped and traced paths end bit-identical to it.
fn canonicalize(state: &mut [Goldilocks; 16]) {
    for e in state.iter_mut() {
        *e = Goldilocks::new(e.as_canonical_u64());
    }
}

/// Apply the Poseidon2 permutation in-place, calling `visitor` once per round.
///
/// Emits 60 callbacks in order: full_round 0–5, partial_round 0–47,
/// full_round 6–11. The initial M_E step is linear and does not emit a
/// callback. The final state is returned canonical, bit-identical to
/// [`permute`].
pub fn permute_traced<V: RoundVisitor>(state: &mut [Goldilocks; 16], visitor: &mut V) {
    let (external, internal) = ROUND_CONSTANTS.split_at(NUM_EXTERNAL);
    mds_light_permutation(state);
    for (round, rc) in external.chunks_exact(16).take(RF_HALF).enumerate() {
        let witnesses = full_round_step(state, rc);
        visitor.full_round(round as u8, state, &witnesses);
    }
    for (round, &c) in internal.iter().enumerate() {
        let witness = partial_round_step(state, c);
        visitor.partial_round(round as u8, state, witness);
    }
    for (round, rc) in external.chunks_exact(16).enumerate().skip(RF_HALF) {
        let witnesses = full_round_step(state, rc);
        visitor.full_round(round as u8, state, &witnesses);
    }
    canonicalize(state);
}

/// `[x², x³]` of an S-box input `x` (the degree-3 witness, see `trace`).
#[inline]
fn sbox_with_witness(x: Goldilocks) -> (Goldilocks, SboxWitnesses) {
    let x2 = x * x;
    let x3 = x2 * x;
    (x3 * (x2 * x2), [x2, x3])
}

/// Add round constants, x⁷ on all 16 elements, M_E.
/// Returns per-element witnesses `[x², x³]` for the pre-S-box state.
#[inline]
fn full_round_step(state: &mut [Goldilocks; 16], rc: &[Goldilocks]) -> [SboxWitnesses; 16] {
    let mut witnesses = [[Goldilocks::ZERO; 2]; 16];
    for i in 0..16 {
        let (y, w) = sbox_with_witness(state[i] + rc[i]);
        state[i] = y;
        witnesses[i] = w;
    }
    mds_light_permutation(state);
    witnesses
}

/// Add the round constant to `state[0]`, x⁷ on it, M_I.
/// Returns the witness `[x², x³]` of the pre-S-box `state[0]`.
#[inline]
fn partial_round_step(state: &mut [Goldilocks; 16], c: Goldilocks) -> SboxWitnesses {
    let (y, w) = sbox_with_witness(state[0] + c);
    state[0] = y;
    matmul_internal(state);
    w
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trace::{FullRoundWitnesses, RoundVisitor, SboxWitnesses};

    struct RoundCounter {
        full: u8,
        partial: u8,
    }

    impl RoundVisitor for RoundCounter {
        fn full_round(&mut self, _: u8, _: &[Goldilocks; 16], _: &FullRoundWitnesses) {
            self.full += 1;
        }
        fn partial_round(&mut self, _: u8, _: &[Goldilocks; 16], _: SboxWitnesses) {
            self.partial += 1;
        }
    }

    #[test]
    fn traced_matches_plain() {
        let mut state_plain = [Goldilocks::new(42); 16];
        let mut state_traced = state_plain;
        let mut counter = RoundCounter {
            full: 0,
            partial: 0,
        };

        permute(&mut state_plain);
        permute_traced(&mut state_traced, &mut counter);

        assert_eq!(state_plain, state_traced);
        assert_eq!(counter.full, 12);
        assert_eq!(counter.partial, 48);
    }

    #[test]
    fn trace_round_indices_are_ordered() {
        struct IndexRecorder {
            full_indices: [u8; 12],
            full_count: usize,
            partial_indices: [u8; 48],
            partial_count: usize,
        }
        impl RoundVisitor for IndexRecorder {
            fn full_round(&mut self, index: u8, _: &[Goldilocks; 16], _: &FullRoundWitnesses) {
                self.full_indices[self.full_count] = index;
                self.full_count += 1;
            }
            fn partial_round(&mut self, index: u8, _: &[Goldilocks; 16], _: SboxWitnesses) {
                self.partial_indices[self.partial_count] = index;
                self.partial_count += 1;
            }
        }

        let mut state = [Goldilocks::ZERO; 16];
        let mut rec = IndexRecorder {
            full_indices: [0; 12],
            full_count: 0,
            partial_indices: [0; 48],
            partial_count: 0,
        };
        permute_traced(&mut state, &mut rec);

        for i in 0..12u8 {
            assert_eq!(rec.full_indices[i as usize], i, "full round index {i}");
        }
        for i in 0..48u8 {
            assert_eq!(
                rec.partial_indices[i as usize], i,
                "partial round index {i}"
            );
        }
    }

    #[test]
    fn sbox_witnesses_are_consistent() {
        // [x², x³] of one x: (x³)² = (x²)³ for every S-box, full and partial.
        struct WitnessChecker {
            checked: usize,
        }
        impl RoundVisitor for WitnessChecker {
            fn full_round(&mut self, _: u8, _: &[Goldilocks; 16], w: &FullRoundWitnesses) {
                for [x2, x3] in w {
                    assert_eq!(*x3 * *x3, *x2 * *x2 * *x2);
                    self.checked += 1;
                }
            }
            fn partial_round(&mut self, _: u8, _: &[Goldilocks; 16], [x2, x3]: SboxWitnesses) {
                assert_eq!(x3 * x3, x2 * x2 * x2);
                self.checked += 1;
            }
        }
        let mut state = [Goldilocks::new(7); 16];
        let mut checker = WitnessChecker { checked: 0 };
        permute_traced(&mut state, &mut checker);
        assert_eq!(checker.checked, 12 * 16 + 48);
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
