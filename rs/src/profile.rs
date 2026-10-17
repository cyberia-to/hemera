// ---
// tags: hemera, rust
// crystal-type: source
// crystal-domain: comp
// ---
//! Permutation profile: the parameters the fast kernels are generic over.
//!
//! A profile fixes the round structure of a width-16 Poseidon2 instance
//! over Goldilocks — full rounds per side, partial rounds, the partial
//! S-box kind, the round constants and the internal diagonal. The
//! external layer is always the Poseidon2 circulant of 4×4 blocks
//! (`field::mds_light_permutation`) and the internal layer always
//! `1 + diag(d)` (`field::matmul_internal`). The live profile is
//! [`HEMERA`] (RF 4+4, RP 16, partial x⁻¹); the kernels take any profile,
//! so a change of round counts or of the partial S-box (e.g. the x⁷
//! RF 6+6 / RP 48 proposal) is a new `Profile` constant, not new code.

use crate::arith::P;
use crate::constants::ROUND_CONSTANTS_U64;
use crate::field::MATRIX_DIAG_16;

/// S-box of the partial rounds (full rounds always use x⁷).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PartialSbox {
    /// x ↦ x^(p−2), 0 ↦ 0.
    Inverse,
    /// x ↦ x⁷.
    Pow7,
}

/// Round structure and constants of a width-16 permutation.
#[derive(Clone, Copy, Debug)]
pub struct Profile<'a> {
    /// Full rounds before (and again after) the partial rounds.
    pub rf_half: usize,
    /// Partial rounds.
    pub rp: usize,
    /// Partial-round S-box.
    pub partial: PartialSbox,
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
        partial: PartialSbox,
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
            partial,
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

/// Number of external constants of the live profile.
const NUM_EXTERNAL: usize = 128;

/// The live hemera profile: RF 4+4, RP 16, partial S-box x⁻¹.
pub const HEMERA: Profile<'static> = Profile::new(
    4,
    16,
    PartialSbox::Inverse,
    ROUND_CONSTANTS_U64.split_at(NUM_EXTERNAL).0,
    ROUND_CONSTANTS_U64.split_at(NUM_EXTERNAL).1,
    &DIAG,
);
