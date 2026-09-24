#!/usr/bin/env python3
"""Universal QF_LIA check of a manually translated Rust reduce128 algorithm.

Z3 is a separate trusted solver here. Its result is not an Eidos theorem and
the Rust-to-integer translation is reviewed manually, not compiler-verified.
"""
import hashlib
import json
from pathlib import Path
import z3

ROOT = Path(__file__).resolve().parents[1]
B, C = 2**64, 2**32
P, E = B-C+1, C-1


def main():
    expected = json.loads((ROOT/'research/model-sources.json').read_text())['rs/src/field.rs']
    if hashlib.sha256((ROOT/'rs/src/field.rs').read_bytes()).hexdigest() != expected:
        raise ValueError('field.rs changed: review the integer translation before updating the fingerprint')
    lo, hl, hh = z3.Ints('lo hi_lo hi_hi')
    assumptions = [lo >= 0, lo < B, hl >= 0, hl < C, hh >= 0, hh < C]
    borrow = z3.If(lo < hh, 1, 0)
    raw_t0 = lo-hh+borrow*B
    t0 = raw_t0-borrow*E
    t1 = hl*E
    total = t0+t1
    carry = z3.If(total >= B, 1, 0)
    wrapped = total-carry*B
    result = wrapped+carry*E
    correction = z3.If(result >= P, 1, 0)
    canonical = result-correction*P
    x = lo+hl*B+hh*(B*C)
    quotient = hl+hh*(C+1)-borrow+carry+correction
    obligations = {
        'overflowing_sub_range': z3.And(raw_t0 >= 0, raw_t0 < B),
        'borrow_correction_does_not_underflow': z3.And(t0 >= 0, t0 < B),
        'constant_product_fits_u64': z3.And(t1 >= 0, t1 < B),
        'sum_fits_one_carry': z3.And(total >= 0, total < 2*B),
        'overflowing_add_range': z3.And(wrapped >= 0, wrapped < B),
        'final_correction_does_not_overflow': z3.And(result >= 0, result < B),
        'canonical_range': z3.And(canonical >= 0, canonical < P),
        'nonnegative_quotient': quotient >= 0,
        'exact_euclidean_identity': x == P*quotient+canonical,
    }
    solver = z3.SolverFor('QF_LIA')
    solver.set(timeout=10000)
    solver.add(*assumptions, z3.Not(z3.And(*obligations.values())))
    smt = solver.to_smt2()
    status = solver.check()
    if status != z3.unsat:
        raise RuntimeError(f'not proved: {status}; {solver.model() if status == z3.sat else solver.reason_unknown()}')
    # A missing high-limb correction must be refutable, not silently accepted.
    bad = z3.SolverFor('QF_LIA')
    bad.set(timeout=10000)
    bad.add(*assumptions, x != P*(quotient-hh)+canonical)
    assert bad.check() == z3.sat
    counterexample = {str(v): bad.model().eval(v).as_long() for v in [lo, hl, hh]}
    (ROOT/'research/reduce128-lia.smt2').write_text(smt)
    print(json.dumps({'scope': 'universal integer model of reduce128; manually translated Rust; Z3 trusted, not Eidos',
        'field_source_sha256': hashlib.sha256((ROOT/'rs/src/field.rs').read_bytes()).hexdigest(),
        'z3_version': z3.get_version_string(), 'logic': 'QF_LIA', 'negated_obligations': str(status),
        'obligations': list(obligations), 'wrong_quotient_counterexample': counterexample,
        'smt2_sha256': hashlib.sha256(smt.encode()).hexdigest()}, indent=2))


if __name__ == '__main__':
    main()
