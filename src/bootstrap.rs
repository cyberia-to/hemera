//! Self-bootstrapping round constant generation.
//!
//! Hemera derives its own round constants from a genesis seed through
//! a zero-constant Poseidon2 sponge (Hemera₀). This module contains
//! the derivation algorithm and a verification test that the static
//! constants in `constants.rs` match the bootstrap output.
//!
//! # Algorithm
//!
//! 1. Create Hemera₀ = Poseidon2 with all 192 round constants = 0
//! 2. Run Hemera₀ as a sponge: absorb GENESIS_SEED with 0x01 padding
//! 3. Squeeze 192 field elements as round constants for the final Hemera

use p3_field::PrimeField64;
use p3_goldilocks::{Goldilocks, Poseidon2Goldilocks};
use p3_symmetric::Permutation;
use rand::RngCore;

use crate::params::{RATE, RATE_BYTES, ROUNDS_F, ROUNDS_P, WIDTH};

/// Genesis seed: five bytes [0x63, 0x79, 0x62, 0x65, 0x72].
///
/// The cryptographic input is this byte sequence alone — no character set,
/// no encoding convention. The fact that these bytes happen to spell "cyber"
/// in ASCII is the human meaning; the specification is the hex literals.
pub(crate) const GENESIS_SEED: &[u8] = &[0x63, 0x79, 0x62, 0x65, 0x72];

/// Create Hemera₀ and return the sponge state after absorbing the genesis seed.
///
/// This is the shared bootstrap logic used by both the CPU verification
/// and the GPU round constant export.
pub(crate) fn bootstrap_sponge_state() -> (Poseidon2Goldilocks<WIDTH>, [Goldilocks; WIDTH]) {
    // Hemera₀ — all-zero round constants.
    let hemera0 = Poseidon2Goldilocks::new_from_rng(ROUNDS_F, ROUNDS_P, &mut ZeroRng);

    // Absorb the genesis seed through Hemera₀ sponge.
    let mut state = [Goldilocks::new(0); WIDTH];

    // Pad: seed || 0x01 || 0x00* to RATE_BYTES (56 bytes).
    let mut padded = [0u8; RATE_BYTES];
    padded[..GENESIS_SEED.len()].copy_from_slice(GENESIS_SEED);
    padded[GENESIS_SEED.len()] = 0x01;

    // Encode padded bytes as rate elements (7 bytes per element).
    let mut rate_block = [Goldilocks::new(0); RATE];
    crate::encoding::bytes_to_rate_block(&padded, &mut rate_block);

    // Absorb via Goldilocks field addition.
    for i in 0..RATE {
        state[i] = state[i] + rate_block[i];
    }

    // Store message length in capacity (state[10]), matching sponge convention.
    state[RATE + 2] = Goldilocks::new(GENESIS_SEED.len() as u64);

    // Permute with Hemera₀.
    hemera0.permute_mut(&mut state);

    (hemera0, state)
}

/// Squeeze the 192 round constants as raw u64 values from the bootstrap sponge.
///
/// Returns the canonical Goldilocks representations in the exact order consumed
/// by `new_from_rng`: 128 external (8 rounds × 16 elements) then 64 internal.
pub(crate) fn bootstrap_constants_u64() -> Vec<u64> {
    let (hemera0, state) = bootstrap_sponge_state();
    let mut rng = SqueezeRng {
        hemera0,
        state,
        buffer: [0u64; RATE],
        pos: RATE,
    };
    let total = ROUNDS_F * WIDTH + ROUNDS_P; // 192
    (0..total).map(|_| rng.next_u64()).collect()
}

/// RNG that produces zeros (used to create Hemera₀).
struct ZeroRng;

impl RngCore for ZeroRng {
    fn next_u32(&mut self) -> u32 {
        0
    }
    fn next_u64(&mut self) -> u64 {
        0
    }
    fn fill_bytes(&mut self, dest: &mut [u8]) {
        dest.fill(0);
    }
}

/// RNG that squeezes Goldilocks elements from a Hemera₀ sponge state.
///
/// Each squeeze extracts RATE (8) elements from the rate portion, then
/// permutes the state for the next block. This produces an unlimited
/// stream of pseudorandom field elements.
struct SqueezeRng {
    hemera0: Poseidon2Goldilocks<WIDTH>,
    state: [Goldilocks; WIDTH],
    buffer: [u64; RATE],
    pos: usize,
}

impl SqueezeRng {
    fn squeeze_block(&mut self) {
        for i in 0..RATE {
            self.buffer[i] = self.state[i].as_canonical_u64();
        }
        self.hemera0.permute_mut(&mut self.state);
        self.pos = 0;
    }
}

impl RngCore for SqueezeRng {
    fn next_u32(&mut self) -> u32 {
        self.next_u64() as u32
    }

    fn next_u64(&mut self) -> u64 {
        if self.pos >= RATE {
            self.squeeze_block();
        }
        let val = self.buffer[self.pos];
        self.pos += 1;
        val
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        let mut written = 0;
        while written < dest.len() {
            let val = self.next_u64();
            let bytes = val.to_le_bytes();
            let remaining = dest.len() - written;
            let n = remaining.min(8);
            dest[written..written + n].copy_from_slice(&bytes[..n]);
            written += n;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::ROUND_CONSTANTS;

    /// Verify that the static constants match the bootstrap derivation.
    ///
    /// This is the critical integrity test: if self-bootstrapping produces
    /// different values than what's in `constants.rs`, the permutation is
    /// broken. Run this after any change to params, encoding, or bootstrap.
    #[test]
    fn static_constants_match_bootstrap() {
        let derived = bootstrap_constants_u64();
        assert_eq!(
            derived.len(),
            ROUND_CONSTANTS.len(),
            "constant count mismatch"
        );
        for (i, (&derived, &static_val)) in derived.iter().zip(ROUND_CONSTANTS.iter()).enumerate() {
            assert_eq!(
                derived, static_val,
                "constant[{i}] mismatch: bootstrap=0x{derived:016X}, static=0x{static_val:016X}"
            );
        }
    }

    #[test]
    fn bootstrap_constants_count() {
        let constants = bootstrap_constants_u64();
        let expected = ROUNDS_F * WIDTH + ROUNDS_P; // 192
        assert_eq!(constants.len(), expected);
    }

    #[test]
    fn bootstrap_constants_deterministic() {
        let c1 = bootstrap_constants_u64();
        let c2 = bootstrap_constants_u64();
        assert_eq!(c1, c2);
    }

    #[test]
    fn bootstrap_constants_nonzero() {
        let constants = bootstrap_constants_u64();
        assert!(constants.iter().any(|&c| c != 0));
    }

    #[test]
    fn bootstrap_constants_are_canonical() {
        let constants = bootstrap_constants_u64();
        let p: u64 = 0xFFFF_FFFF_0000_0001;
        for (i, &c) in constants.iter().enumerate() {
            assert!(c < p, "constant[{i}] = {c} >= p");
        }
    }

    /// Pinned first 4 round constants from self-bootstrapping.
    ///
    /// If this test breaks, the permutation has changed and all downstream
    /// hashes will be different — treat as a breaking change.
    #[test]
    fn bootstrap_pinned_first_constants() {
        let constants = bootstrap_constants_u64();
        assert_eq!(constants[0], 0xD5CCEAC23026433F);
        assert_eq!(constants[1], 0xE3578901A12C12D8);
        assert_eq!(constants[2], 0xF69C218E10D83177);
        assert_eq!(constants[3], 0x580252688A8C5A9D);
    }

    #[test]
    fn squeeze_rng_produces_field_elements() {
        let (hemera0, state) = bootstrap_sponge_state();
        let mut rng = SqueezeRng {
            hemera0,
            state,
            buffer: [0u64; RATE],
            pos: RATE,
        };
        let p: u64 = 0xFFFF_FFFF_0000_0001;
        for _ in 0..100 {
            let val = rng.next_u64();
            assert!(val < p, "squeezed value {val} >= p");
        }
    }

    #[test]
    fn squeeze_rng_fill_bytes() {
        let (hemera0, state) = bootstrap_sponge_state();
        let mut rng = SqueezeRng {
            hemera0,
            state,
            buffer: [0u64; RATE],
            pos: RATE,
        };
        let mut buf = [0u8; 100];
        rng.fill_bytes(&mut buf);
        assert!(buf.iter().any(|&b| b != 0));
    }

    #[test]
    fn genesis_seed_is_cyber() {
        assert_eq!(GENESIS_SEED, b"cyber");
        assert_eq!(GENESIS_SEED, &[0x63, 0x79, 0x62, 0x65, 0x72]);
    }
}
