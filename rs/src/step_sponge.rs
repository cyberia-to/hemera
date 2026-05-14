// ---
// tags: hemera, rust
// crystal-type: source
// crystal-domain: comp
// ---
//! Round-by-round Poseidon2 sponge for STARK trace generation.
//!
//! `StepSponge` implements `Iterator<Item=[Goldilocks; WIDTH]>`, yielding
//! the full post-round state for each of the 24 permutation rounds. Used
//! by nox's pattern-15 trace emitter so each round becomes one trace row.
//!
//! ```text
//! let mut sponge = StepSponge::absorb(&rate_inputs);
//! for state in sponge.by_ref() {
//!     tracer.record(round_row(&state));
//! }
//! tracer.record(squeeze_row(sponge.squeeze()));
//! ```

use crate::field::{Goldilocks, mds_light_permutation};
use crate::params::{RATE, ROUNDS_TOTAL, WIDTH};
use crate::permutation::permute_one_round;

/// Round-by-round Poseidon2 permutation.
///
/// Call `absorb` to initialize, then iterate (or call `step` in a loop)
/// to advance one round at a time. After all 24 rounds, `done()` is true
/// and `squeeze()` returns `state[0]`.
pub struct StepSponge {
    state: [Goldilocks; WIDTH],
    round: usize,
}

impl StepSponge {
    /// Initialize the sponge: XOR `rate` into state[0..rate.len()], apply
    /// the initial MDS, then set round = 0 ready for stepping.
    ///
    /// `rate.len()` must be ≤ `RATE` (8 elements). Capacity elements stay zero.
    pub fn absorb(rate: &[Goldilocks]) -> Self {
        debug_assert!(rate.len() <= RATE, "rate slice longer than RATE");
        let mut state = [Goldilocks::ZERO; WIDTH];
        for (i, &r) in rate.iter().enumerate() {
            state[i] = r;
        }
        // Initial MDS is a linear step applied before the first round.
        mds_light_permutation(&mut state);
        Self { state, round: 0 }
    }

    /// Execute one round and return the post-round state.
    ///
    /// Panics in debug builds if called after `done()`.
    pub fn step(&mut self) -> [Goldilocks; WIDTH] {
        debug_assert!(!self.done(), "step() called after permutation complete");
        permute_one_round(&mut self.state, self.round);
        self.round += 1;
        self.state
    }

    /// Returns `true` when all `ROUNDS_TOTAL` rounds have been executed.
    #[inline]
    pub fn done(&self) -> bool {
        self.round == ROUNDS_TOTAL
    }

    /// Returns `state[0]` — the primary output element — after all rounds.
    ///
    /// Panics in debug builds if called before `done()`.
    pub fn squeeze(&self) -> Goldilocks {
        debug_assert!(self.done(), "squeeze() called before permutation complete");
        self.state[0]
    }
}

impl Iterator for StepSponge {
    type Item = [Goldilocks; WIDTH];

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.done() { None } else { Some(self.step()) }
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use std::vec::Vec;
    use super::*;
    use crate::field::Goldilocks;
    use crate::params::{ROUNDS_TOTAL, WIDTH};
    use crate::permutation::permute;

    fn zero_rate() -> Vec<Goldilocks> {
        std::vec![Goldilocks::ZERO; RATE]
    }

    #[test]
    fn step_count_is_rounds_total() {
        let sponge = StepSponge::absorb(&zero_rate());
        assert_eq!(sponge.count(), ROUNDS_TOTAL);
    }

    #[test]
    fn done_only_after_all_rounds() {
        let mut sponge = StepSponge::absorb(&zero_rate());
        for i in 0..ROUNDS_TOTAL {
            assert!(!sponge.done(), "done() true at round {i}");
            sponge.step();
        }
        assert!(sponge.done());
    }

    #[test]
    fn matches_atomic_permute() {
        // StepSponge stepping all 24 rounds must produce the same final state
        // as a single `permute()` call on the same initial state.
        let rate: Vec<Goldilocks> = (0..RATE as u64).map(Goldilocks::new).collect();

        // Atomic permute
        let mut atomic = [Goldilocks::ZERO; WIDTH];
        for (i, &r) in rate.iter().enumerate() {
            atomic[i] = r;
        }
        permute(&mut atomic);

        // StepSponge
        let mut sponge = StepSponge::absorb(&rate);
        let mut last = [Goldilocks::ZERO; WIDTH];
        for state in sponge.by_ref() {
            last = state;
        }

        assert_eq!(last, atomic, "final states must match");
    }

    #[test]
    fn squeeze_matches_final_state_element_zero() {
        let rate = zero_rate();
        let mut sponge = StepSponge::absorb(&rate);
        let mut final_state = [Goldilocks::ZERO; WIDTH];
        for state in sponge.by_ref() {
            final_state = state;
        }
        assert_eq!(sponge.squeeze(), final_state[0]);
    }

    #[test]
    fn by_ref_leaves_sponge_done_for_squeeze() {
        let mut sponge = StepSponge::absorb(&zero_rate());
        for _ in sponge.by_ref() {}
        assert!(sponge.done());
        // squeeze() should not panic
        let _ = sponge.squeeze();
    }

    #[test]
    fn deterministic() {
        let rate: Vec<Goldilocks> = (0..RATE as u64).map(Goldilocks::new).collect();
        let s1: Vec<_> = StepSponge::absorb(&rate).collect();
        let s2: Vec<_> = StepSponge::absorb(&rate).collect();
        assert_eq!(s1, s2);
    }

    #[test]
    fn different_rate_different_states() {
        let r0 = zero_rate();
        let mut r1 = zero_rate();
        r1[0] = Goldilocks::new(1);

        let s0: Vec<_> = StepSponge::absorb(&r0).collect();
        let s1: Vec<_> = StepSponge::absorb(&r1).collect();
        assert_ne!(s0, s1);
    }

    #[test]
    fn each_round_state_differs() {
        let sponge = StepSponge::absorb(&zero_rate());
        let states: Vec<_> = sponge.collect();
        assert_eq!(states.len(), ROUNDS_TOTAL);
        // Consecutive round states must differ (permutation is not identity on any round).
        for i in 1..states.len() {
            assert_ne!(states[i - 1], states[i], "rounds {} and {} produced identical states", i - 1, i);
        }
    }
}
