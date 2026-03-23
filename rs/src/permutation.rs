// ---
// tags: hemera, rust
// crystal-type: source
// crystal-domain: comp
// ---
//! Poseidon2 permutation for Goldilocks t=16.
//!
//! Hemera parameters: R_F=8 (4+4), R_P=64, d=7.

use crate::constants::ROUND_CONSTANTS;
use crate::field::{Goldilocks, matmul_internal, mds_light_permutation};

/// Number of external (full) round constants: R_F * WIDTH = 8 * 16 = 128.
const NUM_EXTERNAL: usize = 128;

/// Apply the Poseidon2 permutation in-place using the standard Hemera constants.
///
/// Structure: initial MDS → 4 full rounds → 64 partial rounds → 4 full rounds.
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
    // 64 partial rounds: add_rc to state[0] + sbox state[0] + diffusion
    for round in 0..64 {
        state[0] += internal[round];
        state[0] = state[0].pow7();
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

#[cfg(test)]
mod tests {
    use super::*;

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
