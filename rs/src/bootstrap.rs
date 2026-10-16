// ---
// tags: hemera, rust
// crystal-type: source
// crystal-domain: comp
// ---
//! Self-bootstrapping round constant generation.
//!
//! Hemera derives its own round constants from a genesis seed through
//! a zero-constant Poseidon2 sponge (Hemera₀). This module contains
//! the derivation algorithm and a verification test that the static
//! constants in `constants.rs` match the bootstrap output.
//!
//! # Algorithm (`specs/bootstrap.md`)
//!
//! 1. Hemera₀ = the 0.4 permutation (M_E, M_I, x⁷, RF 6+6, RP 48) with all
//!    240 round constants = 0.
//! 2. For counter k = 0, 1, …: seed_k = GENESIS_SEED (k = 0) or
//!    GENESIS_SEED ‖ k as one byte (k ≥ 1). Absorb seed_k through Hemera₀
//!    as a plain-hash sponge (0x01 padding, length in `state[10]`) and
//!    squeeze 240 field elements, 8 per permutation.
//! 3. Accept the first set that passes the constant checks
//!    ([`constant_checks`]): canonical, nonzero, pairwise distinct. The
//!    shipped constants are accepted at k = 0.
//! 4. First 192 elements → external constants, next 48 → internal.

use crate::constants::NUM_CONSTANTS;
use crate::field::{Goldilocks, P};
use crate::params::{RATE, RATE_BYTES, WIDTH};
use crate::permutation::permute_with_constants;

/// Genesis seed: five bytes [0x63, 0x79, 0x62, 0x65, 0x72].
///
/// The cryptographic input is this byte sequence alone — no character set,
/// no encoding convention. The fact that these bytes happen to spell "cyber"
/// in ASCII is the human meaning; the specification is the hex literals.
pub const GENESIS_SEED: &[u8] = &[0x63, 0x79, 0x62, 0x65, 0x72];

/// Counter at which the shipped constants were accepted.
pub const BOOTSTRAP_COUNTER: u8 = 0;

/// All-zero round constants for Hemera₀.
const ZERO_CONSTANTS: [Goldilocks; NUM_CONSTANTS] = [Goldilocks::ZERO; NUM_CONSTANTS];

/// The bootstrap seed for counter `k`: the genesis seed, then `k` as one
/// byte when `k ≥ 1`. Returns the buffer and the seed length.
fn seed_for(k: u8) -> ([u8; 6], usize) {
    let mut seed = [0u8; 6];
    seed[..GENESIS_SEED.len()].copy_from_slice(GENESIS_SEED);
    if k == 0 {
        (seed, GENESIS_SEED.len())
    } else {
        seed[GENESIS_SEED.len()] = k;
        (seed, GENESIS_SEED.len() + 1)
    }
}

/// Hemera₀ sponge state after absorbing `seed` (one padded block).
fn absorb_seed(seed: &[u8]) -> [Goldilocks; WIDTH] {
    debug_assert!(seed.len() < RATE_BYTES);
    let mut state = [Goldilocks::ZERO; WIDTH];

    // Pad: seed || 0x01 || 0x00* to RATE_BYTES (56 bytes).
    let mut padded = [0u8; RATE_BYTES];
    padded[..seed.len()].copy_from_slice(seed);
    padded[seed.len()] = 0x01;

    // Encode padded bytes as rate elements (7 bytes per element).
    let mut rate_block = [Goldilocks::ZERO; RATE];
    crate::encoding::bytes_to_rate_block(&padded, &mut rate_block);

    // Absorb via Goldilocks field addition.
    for i in 0..RATE {
        state[i] += rate_block[i];
    }

    // Store message length in capacity (state[10]), matching sponge convention.
    state[RATE + 2] = Goldilocks::new(seed.len() as u64);

    permute_with_constants(&mut state, &ZERO_CONSTANTS);
    state
}

/// Hemera₀ sponge state after absorbing the genesis seed (counter 0).
pub fn bootstrap_sponge_state() -> [Goldilocks; WIDTH] {
    absorb_seed(GENESIS_SEED)
}

/// Squeeze `NUM_CONSTANTS` elements from Hemera₀ after absorbing `seed`:
/// 8 rate elements per permutation, in the order the permutation consumes
/// them (192 external, then 48 internal).
pub fn squeeze_constants(seed: &[u8]) -> [u64; NUM_CONSTANTS] {
    let mut state = absorb_seed(seed);
    let mut constants = [0u64; NUM_CONSTANTS];
    for (i, c) in constants.iter_mut().enumerate() {
        if i > 0 && i % RATE == 0 {
            permute_with_constants(&mut state, &ZERO_CONSTANTS);
        }
        *c = state[i % RATE].as_canonical_u64();
    }
    constants
}

/// The constant checks of the bootstrap, in order:
/// C1 every constant canonical (< p), C2 every constant nonzero,
/// C3 constants pairwise distinct. Returns one verdict per check.
///
/// The Poseidon2 subspace-trail conditions are conditions on the internal
/// matrix (an M_I whose powers have irreducible characteristic polynomials
/// has no invariant subspace for any constants); they are checked on the
/// matrices themselves (`rs/tests/poseidon2_conditions.rs`).
pub fn constant_checks(c: &[u64; NUM_CONSTANTS]) -> [bool; 3] {
    let canonical = c.iter().all(|&x| x < P);
    let nonzero = c.iter().all(|&x| x != 0);
    let mut sorted = *c;
    sorted.sort_unstable();
    let distinct = sorted.windows(2).all(|w| w[0] != w[1]);
    [canonical, nonzero, distinct]
}

/// Run the bootstrap search: the first counter whose constant set passes
/// every check, with that set; `None` if no counter in 0..=255 does.
pub fn bootstrap() -> Option<(u8, [u64; NUM_CONSTANTS])> {
    (0..=u8::MAX).find_map(|k| {
        let (seed, len) = seed_for(k);
        let c = squeeze_constants(&seed[..len]);
        constant_checks(&c).iter().all(|&ok| ok).then_some((k, c))
    })
}

/// The constants squeezed at [`BOOTSTRAP_COUNTER`] — the shipped round
/// constants (`static_constants_match_bootstrap` checks that the search of
/// [`bootstrap`] accepts exactly this set).
pub fn bootstrap_constants_u64() -> [u64; NUM_CONSTANTS] {
    let (seed, len) = seed_for(BOOTSTRAP_COUNTER);
    squeeze_constants(&seed[..len])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::ROUND_CONSTANTS_U64;

    /// Verify that the static constants match the bootstrap derivation.
    ///
    /// This is the critical integrity test: if self-bootstrapping produces
    /// different values than what's in `constants.rs`, the permutation is
    /// broken. Run this after any change to params, encoding, or bootstrap.
    #[test]
    fn static_constants_match_bootstrap() {
        let (k, derived) = bootstrap().expect("a counter passes the checks");
        assert_eq!(k, BOOTSTRAP_COUNTER, "accepted at a different counter");
        for (i, (&derived, &static_val)) in
            derived.iter().zip(ROUND_CONSTANTS_U64.iter()).enumerate()
        {
            assert_eq!(
                derived, static_val,
                "constant[{i}] mismatch: bootstrap=0x{derived:016X}, static=0x{static_val:016X}"
            );
        }
    }

    #[test]
    fn shipped_constants_pass_every_check() {
        assert_eq!(constant_checks(&ROUND_CONSTANTS_U64), [true; 3]);
    }

    #[test]
    fn checks_reject_bad_sets() {
        let mut c = ROUND_CONSTANTS_U64;
        c[17] = 0;
        assert_eq!(constant_checks(&c), [true, false, true]);
        let mut c = ROUND_CONSTANTS_U64;
        c[239] = c[0];
        assert_eq!(constant_checks(&c), [true, true, false]);
        let mut c = ROUND_CONSTANTS_U64;
        c[5] = P;
        assert!(!constant_checks(&c)[0]);
    }

    #[test]
    fn counter_seeds_differ() {
        assert_eq!(seed_for(0), ([0x63, 0x79, 0x62, 0x65, 0x72, 0], 5));
        assert_eq!(seed_for(1), ([0x63, 0x79, 0x62, 0x65, 0x72, 1], 6));
        assert_ne!(squeeze_constants(&seed_for(1).0[..6]), ROUND_CONSTANTS_U64);
    }

    #[test]
    fn bootstrap_constants_deterministic() {
        assert_eq!(bootstrap_constants_u64(), bootstrap_constants_u64());
    }

    /// Pinned first 4 round constants from self-bootstrapping.
    ///
    /// If this test breaks, the permutation has changed and all downstream
    /// hashes will be different — treat as a breaking change.
    #[test]
    fn bootstrap_pinned_first_constants() {
        let constants = bootstrap_constants_u64();
        assert_eq!(constants[0], 0x8BF262E620238C97);
        assert_eq!(constants[1], 0x41AF509E860145AF);
        assert_eq!(constants[2], 0xEC91406FE4C63B69);
        assert_eq!(constants[3], 0x267C07366E1F063B);
    }

    #[test]
    fn genesis_seed_is_cyber() {
        assert_eq!(GENESIS_SEED, b"cyber");
        assert_eq!(GENESIS_SEED, &[0x63, 0x79, 0x62, 0x65, 0x72]);
    }
}
