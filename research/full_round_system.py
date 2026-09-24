#!/usr/bin/env python3
"""Exact quadratic system for the full hash of a seven-byte message.

This builds/validates witnesses, not an attack or a security proof. Use --system
to export the complete constraints for a solver. Every input bit is constrained.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re

P = 2**64-2**32+1
ROOT = Path(__file__).resolve().parents[1]


class System:
    def __init__(self):
        self.values = [1]
        self.constraints = []
        self.inverse_wires = []

    def variable(self, value):
        self.values.append(value % P)
        return len(self.values)-1

    def constrain(self, *terms):
        # Each term is [coefficient, variable-id, ...], degree at most two.
        assert all(len(term) <= 3 for term in terms)
        self.constraints.append([list(t) for t in terms])

    def linear(self, terms):
        z = self.variable(sum(c*self.values[x] for c, x in terms))
        self.constrain((1, z), *((-c, x) for c, x in terms))
        return z

    def mul(self, a, b):
        z = self.variable(self.values[a]*self.values[b])
        self.constrain((1, z), (-1, a, b))
        return z

    def power7(self, x):
        x2 = self.mul(x, x)
        x3 = self.mul(x2, x)
        x4 = self.mul(x2, x2)
        return self.mul(x3, x4)

    def inverse(self, x):
        y = self.variable(pow(self.values[x], P-2, P))
        z = self.mul(x, y)
        self.constrain((1, x, z), (-1, x))
        self.constrain((1, y, z), (-1, y))
        self.inverse_wires.append((x, y, z))
        return y

    def violations(self, values=None):
        values = self.values if values is None else values
        bad = []
        for i, equation in enumerate(self.constraints):
            result = 0
            for term in equation:
                value = term[0]
                for x in term[1:]:
                    value *= values[x]
                result += value
            if result % P:
                bad.append(i)
        return bad


def parameters():
    expected = json.loads((ROOT/'research/model-sources.json').read_text())
    for path, fingerprint in expected.items():
        if hashlib.sha256((ROOT/path).read_bytes()).hexdigest() != fingerprint:
            raise ValueError(f'{path} changed: review the model before updating model-sources.json')
    field = (ROOT / 'rs/src/field.rs').read_text()
    constants = (ROOT / 'rs/src/constants.rs').read_text()
    block = field.split('pub const MATRIX_DIAG_16:', 1)[1].split('];', 1)[0]
    diag = [int(x, 16) for x in re.findall(r'Goldilocks::new\((0x[0-9a-fA-F]+)\)', block)]
    block = constants.split('pub const ROUND_CONSTANTS_U64:', 1)[1].split('];', 1)[0]
    rc = [int(x, 16) for x in re.findall(r'0x[0-9a-fA-F]+', block)]
    assert len(diag) == 16 and len(rc) == 144
    m4 = [[2, 3, 1, 1], [1, 2, 3, 1], [1, 1, 2, 3], [3, 1, 1, 2]]
    external = [[m4[i % 4][j % 4]*(1+(i//4 == j//4)) for j in range(16)] for i in range(16)]
    internal = [[1+(diag[i] if i == j else 0) for j in range(16)] for i in range(16)]
    return rc, external, internal


def build(message):
    if len(message) != 7:
        raise ValueError('this model is exactly the seven-byte message domain')
    rc, external, internal = parameters()
    s = System()
    s.constrain((1, 0), (-1,)) # wire 0 is the constant one, even for a solver
    bits = [s.variable((int.from_bytes(message, 'little') >> i) & 1) for i in range(56)]
    for bit in bits:
        s.constrain((1, bit, bit), (-1, bit))
    state = [s.linear([(1 << i, bit) for i, bit in enumerate(bits)])]
    # First rate word is message; padding byte is in rate word 1; length is 7.
    state += [s.linear([(1 if i == 1 else 7 if i == 10 else 0, 0)]) for i in range(1, 16)]
    def mix(matrix, state):
        return [s.linear(list(zip(row, state))) for row in matrix]
    state = mix(external, state)
    for round_number in range(24):
        if 4 <= round_number < 20:
            state[0] = s.inverse(s.linear([(1, state[0]), (rc[128+round_number-4], 0)]))
            state = mix(internal, state)
        else:
            full = round_number if round_number < 4 else round_number-16
            state = [s.power7(s.linear([(1, state[i]), (rc[full*16+i], 0)])) for i in range(16)]
            state = mix(external, state)
    assert not s.violations()
    digest = b''.join(s.values[x].to_bytes(8, 'little') for x in state[:4]).hex()
    return s, state[:4], digest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--system', type=Path, help='write the full zero-message system and witness')
    args = parser.parse_args()
    vectors = []
    for message in [bytes(7), bytes([255])*7, b'hemera!', bytes(range(7))]:
        s, outputs, digest = build(message)
        # Every intermediate wire must be constrained, including inverse auxiliaries.
        for wire in range(len(s.values)):
            wrong = s.values[:]
            wrong[wire] = (wrong[wire]+1) % P
            assert s.violations(wrong), f'unconstrained wire {wire}'
        vectors.append({'input_hex': message.hex(), 'digest_hex': digest})
    # Exercise both exceptional and ordinary inverse cases directly.
    for value in [0, 1, 2, P-1]:
        local = System()
        x = local.variable(value)
        y = local.inverse(x)
        assert not local.violations()
        wrong = local.values[:]
        wrong[y] = (wrong[y]+1) % P
        assert local.violations(wrong)
    first, outputs, _ = build(bytes(7))
    if args.system:
        args.system.write_text(json.dumps({'p': P, 'constraints_equal_zero': first.constraints,
            'witness': first.values, 'output_wires': outputs}, indent=2)+'\n')
    sources = ['rs/src/field.rs', 'rs/src/constants.rs', 'rs/src/permutation.rs', 'rs/src/sponge.rs', 'rs/src/encoding.rs']
    print(json.dumps({'scope': 'full 24-round hash on seven-byte inputs; witness validation only; no solver attack',
        'sources_sha256': {p: hashlib.sha256((ROOT/p).read_bytes()).hexdigest() for p in sources},
        'variables': len(first.values), 'equations': len(first.constraints),
        'quadratic_equations': sum(any(len(t) == 3 for t in eq) for eq in first.constraints),
        'inverse_rounds': len(first.inverse_wires),
        'all_zero_message_inverse_inputs_nonzero': all(first.values[x] != 0 for x, _, _ in first.inverse_wires),
        'every_single_wire_perturbation_rejected': True, 'vectors': vectors}, indent=2))


if __name__ == '__main__':
    main()
