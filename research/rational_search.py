#!/usr/bin/env python3
"""Reduced-round one-coordinate preimage search over the actual Goldilocks field.

Uses actual matrices/constants, seven-byte input encoding and explicit zero-branch
recovery. It is NOT a full-digest or full-round attack. python-flint 0.9.0.
"""
import argparse
import hashlib
import json
import subprocess
import sys
import time

from full_round_system import P, ROOT, parameters


def field_roots(polynomial):
    """Extract only linear factors via gcd(f, X^p-X), then split that gcd.

    Factoring all high-degree irreducible factors is unnecessary for preimages.
    The derivative of X^p-X is -1, so the gcd has no repeated factors.
    """
    import flint
    if polynomial.is_zero():
        raise ValueError('zero polynomial has every field element as a root')
    if polynomial.degree() <= 0:
        return set()
    x = flint.nmod_poly([0, 1], P)
    linear_part = polynomial.gcd(x.pow_mod(P, polynomial)-x)
    return {int(root) for root, _ in linear_part.roots()}


def evaluate(x, half, partial):
    rc, external, internal = parameters()
    state = [x, 1]+[0]*8+[7]+[0]*5
    def mix(matrix, values):
        return [sum(a*b for a, b in zip(row, values)) % P for row in matrix]
    state = mix(external, state)
    for full in range(half):
        state = mix(external, [pow((state[i]+rc[16*full+i]) % P, 7, P) for i in range(16)])
    for r in range(partial):
        state[0] = pow((state[0]+rc[128+r]) % P, P-2, P)
        state = mix(internal, state)
    for full in range(8-half, 8):
        state = mix(external, [pow((state[i]+rc[16*full+i]) % P, 7, P) for i in range(16)])
    return state


def search(half, partial, degree_limit, coordinates=4):
    import flint
    bound = 7**(2*half)*2**partial
    if bound > degree_limit:
        return {'half_full_rounds': half, 'partial_rounds': partial, 'status': 'degree_limit',
                'degree_bound': bound, 'degree_limit': degree_limit}
    start = time.monotonic()
    poly = lambda coefficients: flint.nmod_poly(coefficients, P)
    rc, external, internal = parameters()
    nums = [poly([0, 1]), poly([1])]+[poly([0]) for _ in range(8)]+[poly([7])]+[poly([0]) for _ in range(5)]
    den, exceptional = poly([1]), poly([1])
    def mix(matrix, values):
        return [sum((a*b for a, b in zip(row, values)), poly([0])) for row in matrix]
    nums = mix(external, nums)
    for full in range(half):
        nums = mix(external, [(nums[i]+rc[16*full+i]*den)**7 for i in range(16)])
        den = den**7
    for r in range(partial):
        u = nums[0]+rc[128+r]*den
        exceptional *= u
        nums = mix(internal, [den*den]+[n*u for n in nums[1:]])
        den *= u
    for full in range(8-half, 8):
        nums = mix(external, [(nums[i]+rc[16*full+i]*den)**7 for i in range(16)])
        den = den**7
    built = time.monotonic()
    planted = 0x6ed04b17c82a93
    target = evaluate(planted, half, partial)[:coordinates]
    equations = [nums[i]-target[i]*den for i in range(coordinates)]
    assert all(not equation.is_zero() for equation in equations), 'degenerate target equation needs separate handling'
    common = equations[0]
    for equation in equations[1:]:
        common = common.gcd(equation)
    roots = field_roots(common)
    # At an exceptional root the rational equation can omit the true result.
    # Evaluate all such roots with the original total-inverse algorithm too.
    exceptional_roots = field_roots(exceptional)
    candidates = sorted(roots | exceptional_roots)
    valid_field = [x for x in candidates if evaluate(x, half, partial)[:coordinates] == target]
    valid_bytes = [x for x in valid_field if x < 2**56]
    assert planted in valid_bytes
    for x in [0, 1, 17, planted, 2**56-1]:
        for i, value in enumerate(evaluate(x, half, partial)):
            assert int(exceptional(x))*(int(den(x))*value-int(nums[i](x))) % P == 0
    finished = time.monotonic()
    return {'half_full_rounds': half, 'partial_rounds': partial, 'status': 'solved_one_coordinate',
            'coordinates': coordinates, 'degree_bound': bound, 'numerator_degree': nums[0].degree(),
            'denominator_degree': den.degree(), 'equation_degrees': [equation.degree() for equation in equations],
            'common_gcd_degree': common.degree(),
            'exceptional_degree': exceptional.degree(), 'exceptional_roots': len(exceptional_roots),
            'field_preimages': valid_field, 'seven_byte_preimages_hex': [x.to_bytes(7, 'little').hex() for x in valid_bytes],
            'target_coordinates': target, 'build_seconds': round(built-start, 3),
            'solve_and_check_seconds': round(finished-built, 3), 'python_flint_version': flint.__version__,
            'root_algorithm': 'gcd(f, X^p-X), split only linear factors'}


def zero_branch_control():
    """A real pole omitted by the rational equation, recovered through E.

    This control uses a field input; it need not fit the seven-byte domain.
    """
    import flint
    rc, external, internal = parameters()
    u = flint.nmod_poly([external[0][1]+7*external[0][10]+rc[128], external[0][0]], P)
    x = -int(u[0])*pow(int(u[1]), -1, P) % P
    assert int(u(x)) == 0
    # At this first inverse input U=0, rational output numerator is M_I[0][0],
    # while denominator U is zero, so the unsaturated equation misses the input.
    assert internal[0][0] % P != 0
    roots = {int(root) for root, _ in u.roots()}
    assert x in roots
    return {'status': 'passed', 'field_input': x, 'fits_seven_bytes': x < 2**56,
            'actual_output_coordinate': evaluate(x, 0, 1)[0],
            'rational_equation_misses_input': True, 'exceptional_root_recovers_input': True}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--worker', nargs=2, type=int, metavar=('HALF_FULL', 'PARTIAL'))
    parser.add_argument('--degree-limit', type=int, default=100000)
    parser.add_argument('--wall-seconds', type=int, default=45)
    args = parser.parse_args()
    if args.worker:
        half, partial = args.worker
        if not (0 <= half <= 4 and 0 <= partial <= 16):
            parser.error('outside the implemented round ranges')
        print(json.dumps(search(half, partial, args.degree_limit)))
        return
    # Numerical full-profile bridge before doing any reduced-round search.
    vectors = json.loads((ROOT/'research/full-round-system-results.json').read_text())['vectors']
    for v in vectors:
        state = evaluate(int.from_bytes(bytes.fromhex(v['input_hex']), 'little'), 4, 16)
        assert b''.join(x.to_bytes(8, 'little') for x in state[:4]).hex() == v['digest_hex']
    results = []
    zero_control = zero_branch_control()
    for half, partial in [(1, 0), (1, 2), (1, 4), (1, 8), (1, 10), (2, 0), (2, 4), (4, 16)]:
        try:
            run = subprocess.run([sys.executable, '-B', __file__, '--worker', str(half), str(partial),
                '--degree-limit', str(args.degree_limit)], capture_output=True, text=True, timeout=args.wall_seconds)
        except subprocess.TimeoutExpired:
            result = {'half_full_rounds': half, 'partial_rounds': partial, 'status': 'wall_timeout'}
        else:
            if run.returncode:
                raise RuntimeError(run.stderr)
            result = json.loads(run.stdout)
        results.append(result)
        print(f'RF={2*half} RP={partial}: {result["status"]}', file=sys.stderr, flush=True)
    print(json.dumps({'scope': 'reduced-round four-coordinate digest preimages over the seven-byte domain',
        'source_manifest_sha256': hashlib.sha256((ROOT/'research/model-sources.json').read_bytes()).hexdigest(),
        'degree_limit': args.degree_limit, 'wall_seconds_per_profile': args.wall_seconds,
        'full_profile_numerical_bridge': True, 'zero_branch_control': zero_control, 'results': results}, indent=2))


if __name__ == '__main__':
    main()
