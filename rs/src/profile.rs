// ---
// tags: hemera, rust
// crystal-type: source
// crystal-domain: comp
// ---
//! Permutation profile: the parameters the fast kernels are generic over.
//!
//! A profile fixes the round structure of a width-16 Poseidon2 instance
//! over Goldilocks with x⁷ in every round — full rounds per side, partial
//! rounds, the round constants and the internal diagonal. The external
//! layer is always `circ(2·M4, M4, M4, M4)` (`field::mds_light_permutation`)
//! and the internal layer always `1 + diag(d)` (`field::matmul_internal`).
//! The live profile is [`HEMERA`] (RF 6+6, RP 48); other round counts
//! (reduced-round instances of the cryptanalysis campaign, the split
//! benchmark) are other `Profile` values over the same kernel.

use crate::arith::P;
use crate::constants::{NUM_EXTERNAL, ROUND_CONSTANTS_U64};
use crate::field::MATRIX_DIAG_16;
use crate::params::{ROUNDS_F, ROUNDS_P};

/// Round structure and constants of a width-16 permutation.
#[derive(Clone, Copy, Debug)]
pub struct Profile<'a> {
    /// Full rounds before (and again after) the partial rounds.
    pub rf_half: usize,
    /// Partial rounds.
    pub rp: usize,
    /// `2·rf_half` rows of 16 external constants, round-major.
    pub external: &'a [u64],
    /// `rp` internal constants.
    pub internal: &'a [u64],
    /// Internal diagonal `d` of `M_I = 1 + diag(d)`.
    pub diag: &'a [u64; 16],
}

impl<'a> Profile<'a> {
    /// Build a profile; panics (at compile time for `const` profiles) if a
    /// length is wrong or a round constant is not canonical — the kernels
    /// add constants with a single-wrap addition that relies on `c < p`.
    pub const fn new(
        rf_half: usize,
        rp: usize,
        external: &'a [u64],
        internal: &'a [u64],
        diag: &'a [u64; 16],
    ) -> Self {
        assert!(
            external.len() == 2 * rf_half * 16,
            "external constants: 2·rf_half·16"
        );
        assert!(internal.len() == rp, "internal constants: rp");
        let mut i = 0;
        while i < external.len() {
            assert!(external[i] < P, "round constants must be canonical");
            i += 1;
        }
        let mut i = 0;
        while i < internal.len() {
            assert!(internal[i] < P, "round constants must be canonical");
            i += 1;
        }
        Self {
            rf_half,
            rp,
            external,
            internal,
            diag,
        }
    }
}

/// Raw values of `field::MATRIX_DIAG_16`.
const DIAG: [u64; 16] = {
    let mut d = [0u64; 16];
    let mut i = 0;
    while i < 16 {
        d[i] = MATRIX_DIAG_16[i].raw();
        i += 1;
    }
    d
};

/// The live hemera profile 0.4: RF 6+6, RP 48, x⁷ in every round.
pub const HEMERA: Profile<'static> = Profile::new(
    ROUNDS_F / 2,
    ROUNDS_P,
    ROUND_CONSTANTS_U64.split_at(NUM_EXTERNAL).0,
    ROUND_CONSTANTS_U64.split_at(NUM_EXTERNAL).1,
    &DIAG,
);
