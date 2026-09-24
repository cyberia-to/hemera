use cyber_hemera::{
    constants::ROUND_CONSTANTS, field::Goldilocks, permutation::permute_with_constants,
};

#[test]
fn trailing_constants_do_not_change_the_fixed_round_profile() {
    let mut extended = ROUND_CONSTANTS.to_vec();
    extended.extend((0..17).map(|i| Goldilocks::new(i + 1)));

    for seed in [0, 1, u64::MAX] {
        let initial = core::array::from_fn(|i| Goldilocks::new(seed.wrapping_add(i as u64)));
        let mut expected = initial;
        permute_with_constants(&mut expected, &ROUND_CONSTANTS);
        for length in [145, extended.len()] {
            let mut actual = initial;
            permute_with_constants(&mut actual, &extended[..length]);
            assert_eq!(actual, expected, "extra constants changed the round count");
        }
    }
}

#[test]
fn missing_partial_constants_are_rejected() {
    for length in 128..144 {
        let result = std::panic::catch_unwind(|| {
            let mut state = [Goldilocks::ZERO; 16];
            permute_with_constants(&mut state, &ROUND_CONSTANTS[..length]);
        });
        assert!(
            result.is_err(),
            "accepted only {} partial constants",
            length - 128
        );
    }
}
