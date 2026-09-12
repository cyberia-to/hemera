#!/usr/bin/env python3
"""Bounded searches on the full seven-byte hash model, with encoding controls.

SAT means a model was found and rechecked. UNKNOWN is not evidence of security.
Each case runs in its own process with Z3 memory/time limits and a wall deadline.
"""
import argparse
import json
from pathlib import Path
import subprocess
import sys
import time

from full_round_system import P, System, build

CASES = ['inverse_zero_valid', 'inverse_zero_invalid', 'inverse_nonzero_invalid',
         'full_witness_valid', 'full_witness_wrong_digest', 'full_preimage', 'full_collision']


def worker(case, timeout_ms):
    import z3
    z3.set_param('memory_max_size', 512)
    solver = z3.Solver()
    solver.set(timeout=timeout_ms)
    systems = []

    def encode(system, prefix):
        vs = [z3.Int(f'{prefix}_{i}') for i in range(len(system.values))]
        solver.add(*(z3.And(v >= 0, v < P) for v in vs))
        solver.add(vs[0] == 1)
        for equation in system.constraints:
            terms = []
            for term in equation:
                expression = term[0]
                for i in term[1:]:
                    expression *= vs[i]
                terms.append(expression)
            solver.add(sum(terms) % P == 0)
        systems.append((system, vs))
        return vs

    expected = None
    metadata = {}
    if case.startswith('inverse_'):
        system = System()
        x = system.variable(0)
        y = system.inverse(x)
        vs = encode(system, 'inv')
        xv, yv = {'inverse_zero_valid': (0, 0), 'inverse_zero_invalid': (0, 1),
                  'inverse_nonzero_invalid': (1, 0)}[case]
        solver.add(vs[x] == xv, vs[y] == yv)
        expected = 'sat' if case == 'inverse_zero_valid' else 'unsat'
    else:
        # The preimage fixture is not supplied as solver variable assignments.
        message = bytes.fromhex('932ac8174bd06e')
        system, outputs, digest = build(message)
        vs = encode(system, 'a')
        metadata['target_hex'] = digest
        if case == 'full_collision':
            second, outputs2, _ = build(bytes(7))
            ws = encode(second, 'b')
            solver.add(*(vs[a] == ws[b] for a, b in zip(outputs, outputs2)))
            solver.add(z3.Or(*(vs[i] != ws[i] for i in range(1, 57))))
            metadata.pop('target_hex')
        else:
            target = [system.values[i] for i in outputs]
            if case == 'full_witness_wrong_digest':
                target[0] = (target[0]+1) % P
            solver.add(*(vs[i] == value for i, value in zip(outputs, target)))
            if case.startswith('full_witness_'):
                solver.add(*(v == value for v, value in zip(vs, system.values)))
                expected = 'sat' if case == 'full_witness_valid' else 'unsat'
    start = time.monotonic()
    try:
        result = solver.check()
    except z3.Z3Exception as error:
        if 'out of memory' not in str(error):
            raise
        if expected is not None:
            raise RuntimeError(f'control exhausted memory: {case}') from error
        print(json.dumps({'case': case, 'status': 'resource_exhausted',
            'reason': 'Z3 memory limit', 'solver_seconds': round(time.monotonic()-start, 3),
            'z3_version': z3.get_version_string(), **metadata}), flush=True)
        return
    answer = {'case': case, 'status': str(result), 'expected_control': expected,
              'solver_seconds': round(time.monotonic()-start, 3),
              'z3_version': z3.get_version_string(), **metadata}
    if result == z3.unknown:
        answer['reason'] = solver.reason_unknown()
    elif result == z3.sat:
        model = solver.model()
        messages = []
        for system, variables in systems:
            values = [model.eval(v, model_completion=True).as_long() for v in variables]
            assert not system.violations(values), 'solver model fails original equations'
            assert values[0] == 1 and all(0 <= x < P for x in values)
            if not case.startswith('inverse_'):
                assert all(values[i] in [0, 1] for i in range(1, 57))
                n = sum(values[i+1] << i for i in range(56))
                message = n.to_bytes(7, 'little')
                actual_system, actual_outputs, actual_digest = build(message)
                assert all(values[i] == actual_system.values[i] for i in range(len(values)))
                messages.append({'input_hex': message.hex(), 'digest_hex': actual_digest})
        if messages:
            answer['candidates'] = messages
            if case == 'full_preimage':
                assert messages[0]['digest_hex'] == digest
            if case == 'full_collision':
                assert messages[0]['input_hex'] != messages[1]['input_hex']
                assert messages[0]['digest_hex'] == messages[1]['digest_hex']
    if expected is not None and str(result) != expected:
        raise RuntimeError(f'encoding control failed: {answer}')
    print(json.dumps(answer), flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--worker', choices=CASES)
    parser.add_argument('--timeout-ms', type=int, default=10000)
    args = parser.parse_args()
    if args.timeout_ms <= 0:
        parser.error('timeout must be positive')
    if args.worker:
        worker(args.worker, args.timeout_ms)
        return
    results = []
    for case in CASES:
        try:
            run = subprocess.run([sys.executable, '-B', str(Path(__file__).resolve()),
                '--worker', case, '--timeout-ms', str(args.timeout_ms)], text=True,
                capture_output=True, timeout=args.timeout_ms/1000+20)
        except subprocess.TimeoutExpired:
            if case not in ['full_preimage', 'full_collision']:
                raise RuntimeError(f'control did not complete: {case}')
            results.append({'case': case, 'status': 'wall_timeout'})
            continue
        if run.returncode:
            raise RuntimeError(f'{case} failed: {run.stderr[-3000:]}')
        result = json.loads(run.stdout)
        results.append(result)
        print(f'{case}: {result["status"]}', file=sys.stderr, flush=True)
    print(json.dumps({'scope': 'full seven-byte hash searches; bounded solver, no security inference from unknown',
        'timeout_ms_per_case': args.timeout_ms, 'memory_limit_mb_per_worker': 512,
        'results': results}, indent=2))


if __name__ == '__main__':
    main()
