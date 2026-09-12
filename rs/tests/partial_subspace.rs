//! Bridge the exact affine certificate to the actual Rust partial rounds.
//! This deliberately excludes the full rounds and sponge input constraints.
use cyber_hemera::{
    constants::ROUND_CONSTANTS,
    field::{Goldilocks, matmul_internal},
};

include!("fixtures/partial_subspace.rs");

#[test]
fn fifteen_partial_rounds_match_affine_certificate() {
    for z in [0, 1, 2, u64::MAX, 0x123456789abcdef] {
        let z = Goldilocks::new(z);
        let mut state =
            core::array::from_fn(|i| Goldilocks::new(BASE[i]) + z * Goldilocks::new(DIRECTION[i]));
        for rc in &ROUND_CONSTANTS[128..143] {
            state[0] += *rc;
            assert_eq!(state[0].as_canonical_u64(), 1);
            state[0] = state[0].inv();
            matmul_internal(&mut state);
        }
        for i in 0..16 {
            let expected =
                Goldilocks::new(OUTPUT_BASE[i]) + z * Goldilocks::new(OUTPUT_DIRECTION[i]);
            assert_eq!(state[i].as_canonical_u64(), expected.as_canonical_u64());
        }
    }
    // The restriction is essential; perturbing coordinate zero violates it.
    let perturbed = Goldilocks::new(BASE[0]) + Goldilocks::new(1) + ROUND_CONSTANTS[128];
    assert_ne!(perturbed.as_canonical_u64(), 1);
}
