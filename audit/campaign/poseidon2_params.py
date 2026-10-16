"""Poseidon2 parameter generation and checks for hemera 0.4, pure Python.

`python3 poseidon2_params.py` reproduces, with no dependency:

1. the Grain LFSR of the Poseidon / Poseidon2 parameter scripts
   (HorizenLabs/poseidon2 `poseidon2_rust_params.sage`), checked against
   the Horizen Labs width-8 Goldilocks round constants (as shipped by
   Plonky3 `p3-goldilocks`, HL_GOLDILOCKS_8_*);
2. the reference internal diagonals: for t = 8 and t = 16 the Grain stream
   (field 1, S-box 0, n 64, R_F 8, R_P 22) after the round constants
   proposes diagonals 16 limbs at a time; the first M_I = 1 + diag(d) whose
   powers M_I^1 .. M_I^(2t) all have an irreducible characteristic
   polynomial is taken. This yields the published MATRIX_DIAG_8/16 exactly
   (t = 16: candidate 46, after 46 rejections);
3. the matrix checks on the 0.4 linear layers (M4 MDS, M_E and M_I
   invertible, the minimal-polynomial condition on M_I^i, i = 1 .. 32);
4. the constant checks used by the 0.4 bootstrap (`constant_checks`).

Arithmetic over F_p with Python integers; characteristic polynomials by
Faddeev-LeVerrier (p > t, so the divisions by k <= 16 exist);
irreducibility by Rabin's test.
"""
import itertools

P = 2**64 - 2**32 + 1

# Poseidon2 paper, Appendix B; Horizen Labs reference; Plonky3 HLMDSMat4.
M4_HL = [[5, 7, 1, 3], [4, 6, 1, 1], [1, 3, 5, 7], [1, 1, 4, 6]]
# Plonky3 MDSMat4 — the hemera 0.3 block (the record only).
M4_PLONKY3 = [[2, 3, 1, 1], [1, 2, 3, 1], [1, 1, 2, 3], [3, 1, 1, 2]]

# MAT_DIAG_M_1 of the reference instances: M_I = 1 + diag(d) (all-ones
# matrix plus the diagonal). Re-derived by `derive_diag` below.
DIAG8 = [0xa98811a1fed4e3a5, 0x1cc48b54f377e2a0, 0xe40cd4f6c5609a26, 0x11de79ebca97a4a3,
         0x9177c73d8b7e929c, 0x2a6fe8085797e791, 0x3de6e93329f8d5ad, 0x3f7af9125da962fe]
DIAG16 = [0xde9b91a467d6afc0, 0xc5f16b9c76a9be17, 0x0ab0fef2d540ac55, 0x3001d27009d05773,
          0xed23b1f906d3d9eb, 0x5ce73743cba97054, 0x1c3bab944af4ba24, 0x2faa105854dbafae,
          0x53ffb3ae6d421a10, 0xbcda9df8884ba396, 0xfc1273e4a31807bb, 0xc77952573d5142c0,
          0x56683339a819b85e, 0x328fcbd8f0ddc8eb, 0xb5101e303fce9cb7, 0x774487b8c40089bb]

# Horizen Labs / Plonky3 width-8 test vectors (p3-goldilocks poseidon2.rs tests).
HL8_VECTORS = [
    ([0] * 8,
     [4214787979728720400, 12324939279576102560, 10353596058419792404, 15456793487362310586,
      10065219879212154722, 16227496357546636742, 2959271128466640042, 14285409611125725709]),
    (list(range(8)),
     [14266028122062624699, 5353147180106052723, 15203350112844181434, 17630919042639565165,
      16601551015858213987, 10184091939013874068, 16774100645754596496, 12047415603622314780]),
    ([5116996373749832116, 8931548647907683339, 17132360229780760684, 11280040044015983889,
      11957737519043010992, 15695650327991256125, 17604752143022812942, 543194415197607509],
     [1831346684315917658, 13497752062035433374, 12149460647271516589, 15656333994315312197,
      4671534937670455565, 3140092508031220630, 4251208148861706881, 6973971209430822232]),
]

# First external row and first internal constant of HL_GOLDILOCKS_8.
HL8_EXT_ROW0 = [0xdd5743e7f2a5a5d9, 0xcb3a864e58ada44b, 0xffa2449ed32f8cdc, 0x42025f65d6bd13ee,
                0x7889175e25506323, 0x34b98bb03d24b737, 0xbdcc535ecc4faa2a, 0x5b20ad869fc0d033]
HL8_INT = [0x488897d85ff51f56, 0x1140737ccb162218, 0xa7eeb9215866ed35]
HL8_LAST = 0x95f2394459fbc25e   # last external constant


# ── Grain LFSR (Poseidon parameter scripts) ─────────────────────────

def grain(t, rf, rp, field=1, sbox=0, n=64):
    bits = [int(b) for b in (bin(field)[2:].zfill(2) + bin(sbox)[2:].zfill(4) + bin(n)[2:].zfill(12)
                             + bin(t)[2:].zfill(12) + bin(rf)[2:].zfill(10) + bin(rp)[2:].zfill(10))]
    bits += [1] * 30

    def step():
        b = bits[62] ^ bits[51] ^ bits[38] ^ bits[23] ^ bits[13] ^ bits[0]
        bits.pop(0)
        bits.append(b)
        return b

    for _ in range(160):
        step()
    while True:
        b = step()
        while b == 0:          # self-shrinking: keep the second bit of each pair whose first bit is 1
            step()
            b = step()
        yield step()


def grain_bits(g, n):
    v = 0
    for _ in range(n):
        v = (v << 1) | next(g)
    return v


def grain_constants(t, rf, rp, g=None):
    """R_F * t + R_P constants by rejection sampling (< p), in the Poseidon2
    script's order: rf/2 external rows, rp internal, rf/2 external rows."""
    g = g or grain(t, rf, rp)
    out = []
    for _ in range(rf * t + rp):
        v = grain_bits(g, 64)
        while v >= P:
            v = grain_bits(g, 64)
        out.append(v)
    return out


# ── linear algebra over F_p ─────────────────────────────────────────

def matmul(a, b):
    n, m, k = len(a), len(b[0]), len(b)
    return [[sum(a[i][x] * b[x][j] for x in range(k)) % P for j in range(m)] for i in range(n)]


def det(m):
    m = [row[:] for row in m]
    n, d = len(m), 1
    for c in range(n):
        piv = next((r for r in range(c, n) if m[r][c] % P), None)
        if piv is None:
            return 0
        if piv != c:
            m[c], m[piv] = m[piv], m[c]
            d = -d
        d = d * m[c][c] % P
        inv = pow(m[c][c], P - 2, P)
        for r in range(c + 1, n):
            f = m[r][c] * inv % P
            if f:
                m[r] = [(x - f * y) % P for x, y in zip(m[r], m[c])]
    return d % P


def charpoly(a):
    """Coefficients c_0 .. c_n (c_n = 1) of det(xI - A), Faddeev-LeVerrier."""
    n = len(a)
    c = [0] * (n + 1)
    c[n] = 1
    m = [[0] * n for _ in range(n)]
    for k in range(1, n + 1):
        am = matmul(a, m)
        m = [[(am[i][j] + (c[n - k + 1] if i == j else 0)) % P for j in range(n)] for i in range(n)]
        am = matmul(a, m)
        c[n - k] = (-sum(am[i][i] for i in range(n)) * pow(k, P - 2, P)) % P
    return c


def _pmod(a, f):
    a, n = a[:], len(f) - 1
    for i in range(len(a) - 1, n - 1, -1):
        q = a[i]
        if q:
            for j in range(n + 1):
                a[i - n + j] = (a[i - n + j] - q * f[j]) % P
    return (a[:n] + [0] * n)[:n]


def _pmulmod(a, b, f):
    r = [0] * (len(a) + len(b) - 1)
    for i, x in enumerate(a):
        if x:
            for j, y in enumerate(b):
                r[i + j] = (r[i + j] + x * y) % P
    return _pmod(r, f)


def _ppowmod(a, e, f):
    r = [1] + [0] * (len(f) - 2)
    while e:
        if e & 1:
            r = _pmulmod(r, a, f)
        a = _pmulmod(a, a, f)
        e >>= 1
    return r


def _trim(a):
    a = a[:]
    while a and a[-1] == 0:
        a.pop()
    return a


def _pgcd(a, b):
    a, b = _trim(a), _trim(b)
    while b:
        inv = pow(b[-1], P - 2, P)
        while len(a) >= len(b):
            q, s = a[-1] * inv % P, len(a) - len(b)
            for j in range(len(b)):
                a[s + j] = (a[s + j] - q * b[j]) % P
            a = _trim(a)
        a, b = b, a
    return a


def irreducible(f):
    """Rabin: monic f of degree n is irreducible over F_p iff
    x^(p^n) = x mod f and gcd(x^(p^(n/q)) - x, f) = 1 for every prime q | n."""
    n = len(f) - 1
    x = [0, 1] + [0] * (n - 2)
    frob, xk = {}, x
    for k in range(1, n + 1):
        xk = _ppowmod(xk, P, f)
        frob[k] = xk
    if _trim([(a - b) % P for a, b in zip(frob[n], x)]):
        return False
    for q in (q for q in range(2, n + 1) if n % q == 0 and all(q % r for r in range(2, q))):
        if len(_pgcd(f, [(a - b) % P for a, b in zip(frob[n // q], x)])) > 1:
            return False
    return True


def internal_matrix(diag):
    t = len(diag)
    return [[(diag[i] + 1) % P if i == j else 1 for j in range(t)] for i in range(t)]


def external_matrix(m4, t=16):
    k = t // 4
    return [[(2 if bi == bj else 1) * m4[i % 4][j % 4] % P
             for j in range(t) for bj in [j // 4]]
            for i in range(t) for bi in [i // 4]]


def minpoly_condition(m, periods):
    """First i in 1..periods where charpoly(M^i) is reducible, or None.
    An irreducible charpoly of degree t is also the minimal polynomial."""
    mi = m
    for i in range(1, periods + 1):
        if not irreducible(charpoly(mi)):
            return i
        mi = matmul(m, mi)
    return None


def is_mds(m):
    n = len(m)
    for k in range(1, n + 1):
        for rows in itertools.combinations(range(n), k):
            for cols in itertools.combinations(range(n), k):
                if det([[m[r][c] for c in cols] for r in rows]) == 0:
                    return False
    return True


def derive_diag(t, rf=8, rp=22):
    """The Poseidon2 script's internal diagonal: after the round constants,
    draw t raw 64-bit Grain values r_i (no rejection; reduced mod p), form
    M = (J - I) + diag(r) and keep the first M whose powers 1 .. 2t satisfy
    the minimal-polynomial condition. Returns (d = r - 1, rejected count)."""
    g = grain(t, rf, rp)
    grain_constants(t, rf, rp, g)
    for cand in itertools.count():
        r = [grain_bits(g, 64) % P for _ in range(t)]
        d = [(x - 1) % P for x in r]
        if det(internal_matrix(d)) and minpoly_condition(internal_matrix(d), 2 * t) is None:
            return d, cand


def matrix_checks(m4=M4_HL, diag=DIAG16, t=16):
    me, mi = external_matrix(m4, t), internal_matrix(diag)
    return [
        ('M4 MDS (all 69 square minors nonzero)', is_mds(m4)),
        ('M_E invertible', det(me) != 0),
        ('M_I invertible', det(mi) != 0),
        (f'charpoly(M_I^i) irreducible, i = 1..{2 * t}', minpoly_condition(mi, 2 * t) is None),
    ]


def constant_checks(rc):
    """Acceptance checks on a bootstrapped constant set (specs/bootstrap.md)."""
    return [
        ('C1 canonical (< p)', all(0 <= c < P for c in rc)),
        ('C2 nonzero', all(c != 0 for c in rc)),
        ('C3 pairwise distinct', len(set(rc)) == len(rc)),
    ]


if __name__ == '__main__':
    import time
    rc8 = grain_constants(8, 8, 22)
    ok = rc8[:8] == HL8_EXT_ROW0 and rc8[32:35] == HL8_INT and rc8[-1] == HL8_LAST
    print('Grain reproduces the HL width-8 Goldilocks round constants:', ok)
    for t, ref in ((8, DIAG8), (16, DIAG16)):
        t0 = time.time()
        d, rejected = derive_diag(t)
        print(f'diagonal t={t}: Grain candidate {rejected} (after {rejected} rejections) '
              f'equals the reference MAT_DIAG{t}_M_1: {d == ref}  ({time.time() - t0:.0f} s)')
    me = external_matrix(M4_HL)
    print('M_E = circ(2 M4, M4, M4, M4) row 0:', me[0])
    for name, ok in matrix_checks():
        print(f'  {name}: {ok}')
    print('0.3 block circ(2,3,1,1) MDS:', is_mds(M4_PLONKY3))
