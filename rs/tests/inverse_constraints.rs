use cyber_hemera::{
    field::{Goldilocks, P},
    trace::inverse_residuals,
};

fn accepts(x: Goldilocks, y: Goldilocks) -> bool {
    inverse_residuals(x, y)
        .iter()
        .all(|v| v.as_canonical_u64() == 0)
}

#[test]
fn rejects_the_previously_accepted_zero_output_for_nonzero_input() {
    let x = Goldilocks::new(1);
    let y = Goldilocks::ZERO;
    assert_eq!((x * y * (x * y - Goldilocks::new(1))).as_canonical_u64(), 0);
    assert_eq!(((Goldilocks::new(1) - x * y) * y).as_canonical_u64(), 0);
    assert!(!accepts(x, y));
}

#[test]
fn total_inverse_witnesses_accept_and_altered_outputs_reject() {
    let mut inputs = vec![0, 1, 2, P - 1, P, P + 1, u64::MAX];
    let mut raw = 0x9e3779b97f4a7c15u64;
    for _ in 0..256 {
        raw = raw.wrapping_mul(6364136223846793005).wrapping_add(1);
        inputs.push(raw);
    }
    for raw in inputs {
        let x = Goldilocks::new(raw);
        let y = x.inv();
        assert!(accepts(x, y));
        assert!(!accepts(x, y + Goldilocks::new(1)));
        if x.as_canonical_u64() != 0 {
            assert!(!accepts(x, Goldilocks::ZERO));
        }
    }
    assert!(accepts(Goldilocks::new(P), Goldilocks::new(P)));
}
