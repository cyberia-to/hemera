#!/usr/bin/env python3
"""Reproducible algebraic checks; not a security proof or a Rust refinement.

Run with Python 3 and SymPy 1.14: python3 research/inverse_sbox.py
"""
import hashlib
import json
import math
from pathlib import Path
import re

import sympy as sp

P = 2**64 - 2**32 + 1
ROOT = Path(__file__).resolve().parents[1]


def old_constraints(x, y, p):
    return (x*y*(x*y-1)) % p == 0 and ((1-x*y)*y) % p == 0


def cubic_constraints(x, y, p):
    return (x*(x*y-1)) % p == 0 and (y*(x*y-1)) % p == 0


def quadratic_constraints(x, y, z, p):
    return (x*y-z) % p == 0 and (x*(z-1)) % p == 0 and (y*(z-1)) % p == 0


def chain(source):
    """Interpret the actual multiplication-only Rust addition-chain syntax.

    Track formal exponents and multiplication depth, without executing field
    arithmetic. Reject any unrecognized statement rather than skipping it.
    """
    body = source.split("let x = self;", 1)[1].split("/// Double", 1)[0]
    body = re.sub(r"//[^\n]*", "", body).strip()
    body = body.rsplit("}", 1)[0].strip()
    values = {"x": (1, 0)}
    operations = 0
    while body:
        loop = re.match(r"for _ in 0\.\.(\d+)\s*\{\s*t = t \* t;\s*\}", body)
        if loop:
            n = int(loop[1])
            e, d = values["t"]
            values["t"] = (e * 2**n, d+n)
            operations += n
            body = body[loop.end():].strip()
            continue
        assignment = re.match(r"(?:let (?:mut )?)?(\w+) = (\w+)(?: \* (\w+))?;", body)
        if assignment:
            dst, a, b = assignment.groups()
            ea, da = values[a]
            if b:
                eb, db = values[b]
                values[dst] = (ea+eb, max(da, db)+1)
                operations += 1
            else:
                values[dst] = values[a]
            body = body[assignment.end():].strip()
            continue
        final = re.fullmatch(r"(\w+) \* (\w+)", body)
        if not final:
            raise ValueError(f"unsupported chain syntax: {body[:100]}")
        a, b = (values[name] for name in final.groups())
        return {"exponent": a[0]+b[0], "multiplications": operations+1,
                "multiplicative_depth": max(a[1], b[1])+1}
    raise ValueError("missing return expression")


def rank_mod(rows, p):
    a = [[int(x) % p for x in row] for row in rows]
    rank = 0
    for col in range(len(a[0])):
        pivot = next((i for i in range(rank, len(a)) if a[i][col]), None)
        if pivot is None:
            continue
        a[rank], a[pivot] = a[pivot], a[rank]
        inverse = pow(a[rank][col], -1, p)
        a[rank] = [x*inverse % p for x in a[rank]]
        for i in range(rank+1, len(a)):
            scale = a[i][col]
            a[i] = [(x-scale*y) % p for x, y in zip(a[i], a[rank])]
        rank += 1
        if rank == len(a):
            break
    return rank


def main():
    source = (ROOT / "rs/src/field.rs").read_text()
    addition_chain = chain(source)
    assert addition_chain["exponent"] == P-2
    assert old_constraints(1, 0, P)
    assert not cubic_constraints(1, 0, P)
    fields = []
    for p in [17, 97, 193, 257, 769]:
        assert sp.isprime(p) and math.gcd(7, p-1) == 1
        inverse = [pow(x, p-2, p) for x in range(p)]
        old_false = 0
        for x in range(p):
            for y in range(p):
                expected = y == inverse[x]
                old_false += old_constraints(x, y, p) and not expected
                assert cubic_constraints(x, y, p) == expected
                # The first quadratic equation forces z = x*y.
                assert quadratic_constraints(x, y, x*y % p, p) == expected
        def ddt(sbox):
            maximum = 0
            for a in range(1, p):
                counts = [0]*p
                for x in range(p):
                    counts[(sbox[(x+a) % p]-sbox[x]) % p] += 1
                maximum = max(maximum, max(counts))
            return maximum
        fields.append({"p": p, "old_false_acceptances": old_false,
                       "inverse_differential_uniformity": ddt(inverse),
                       "power7_differential_uniformity": ddt([pow(x, 7, p) for x in range(p)]),
                       "corrected_constraints_exhaustive": True})

    # For difference a=1, b=1 the exceptional x=0,-1 both solve the
    # derivative equation; the two roots of x^2+x+1=0 supply two more.
    roots = sp.sqrt_mod(P-3, P, all_roots=True)
    witnesses = sorted({0, P-1, *((int(r)-1)*pow(2, -1, P) % P for r in roots)})
    assert len(witnesses) == 4
    assert all((pow((x+1) % P, P-2, P)-pow(x, P-2, P)) % P == 1 for x in witnesses)

    block = source.split("pub const MATRIX_DIAG_16:", 1)[1].split("];", 1)[0]
    diag = [int(x, 16) for x in re.findall(r"Goldilocks::new\((0x[0-9a-fA-F]+)\)", block)]
    assert len(diag) == 16
    matrix = [[(1 + (diag[i] if i == j else 0)) % P for j in range(16)] for i in range(16)]
    rows = [[1] + [0]*15]
    for _ in range(15):
        rows.append([sum(rows[-1][i]*matrix[i][j] for i in range(16)) % P for j in range(16)])
    ranks = [rank_mod(rows[:i], P) for i in range(1, 17)]
    x = sp.Symbol("X")
    characteristic = sp.Poly(sp.Matrix(matrix).charpoly(x).as_expr(), x, modulus=P)
    factors = sp.factor_list(characteristic)[1]
    report = {
        "scope": "algebraic checks only; no full-round cryptanalysis or security certification",
        "sympy_version": sp.__version__,
        "field_source_sha256": hashlib.sha256(source.encode()).hexdigest(),
        "p": P, "p_is_prime": bool(sp.isprime(P)),
        "gcd_7_p_minus_1": math.gcd(7, P-1),
        "addition_chain": addition_chain,
        "old_constraint_counterexample": {"x": 1, "y": 0},
        "small_fields": fields,
        "goldilocks_inverse_ddt_a1_b1_witnesses": witnesses,
        "nonzero_branch_projective_degree_upper_bound": 7**8 * 2**16,
        "internal_matrix_rank": rank_mod(matrix, P),
        "inactive_coordinate_observability_ranks": ranks,
        "internal_charpoly_factor_degrees": [[f.degree(), multiplicity] for f, multiplicity in factors],
        "internal_charpoly_coefficients_mod_p": [int(c) % P for c in characteristic.all_coeffs()],
    }
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
