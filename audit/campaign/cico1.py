"""CICO-1 on reduced-round hemera by univariate rational interpolation.

One free rate element X; every other input element fixed; ask for output
limb `limb` = target. The state is carried symbolically as numerators N_i(X)
over one common denominator D(X):
  full round   x_i -> (x_i + c)^7          N_i' = (N_i + cD)^7, D' = D^7
  partial      x_0 -> (x_0 + c)^-1         U = N_0 + cD; N_0' = D^2,
                                           N_i' = N_i U, D' = D U
so a partial inverse round doubles the degree and a full x^7 round
multiplies it by seven: deg <= 7^(2 rf_half) * 2^rp.
Roots of N_limb - target*D are candidate inputs; each is checked against
the concrete permutation of hemera_ref.py (the model verified against
hemera's vectors).
Root finding: gcd(f, X^p - X) by one modular powering, then the roots of the
gcd (python-flint nmod_poly).
usage: python3 cico1.py rf_half rp [limb] [target]   (needs python-flint)
"""
import sys, os, time, random
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import flint
from hemera_ref import P, RC, DIAG, permute

rf_half, rp = int(sys.argv[1]), int(sys.argv[2])
limb = int(sys.argv[3]) if len(sys.argv) > 3 else 0
target = int(sys.argv[4]) if len(sys.argv) > 4 else 0

R = lambda coeffs: flint.nmod_poly(coeffs, P)
rng = random.Random(20261016)
fixed = [rng.randrange(1 << 56) for _ in range(8)]
cap = [0, 0, 168, 0, 0, 0, 0, 0]          # counter, flags, msg_length, domain, ns, ns, -, -
base = fixed + cap
FREE = 3                                   # position of X in the rate


def m4(a, b, c, d):
    return (2*a + 3*b + c + d, a + 2*b + 3*c + d, a + b + 2*c + 3*d, 3*a + b + c + 2*d)


def m_e(s):
    out = []
    for j in range(4):
        out.extend(m4(*s[4*j:4*j+4]))
    sums = [out[k] + out[4 + k] + out[8 + k] + out[12 + k] for k in range(4)]
    return [out[i] + sums[i % 4] for i in range(16)]


def m_i(s):
    t = s[0]
    for x in s[1:]:
        t = t + x
    return [s[i] * DIAG[i] + t for i in range(16)]


t0 = time.time()
N = [R([base[i]]) for i in range(16)]
N[FREE] = R([0, 1])
D = R([1])
N = m_e(N)
ext, intl = RC[:128], RC[128:]
for r in range(rf_half):
    N = [(N[i] + ext[r*16 + i] * D) ** 7 for i in range(16)]
    D = D ** 7
    N = m_e(N)
for j in range(rp):
    U = N[0] + intl[j] * D
    N = [D * D] + [N[i] * U for i in range(1, 16)]
    D = D * U
    N = m_i(N)
for k in range(rf_half):
    N = [(N[i] + ext[(4 + k)*16 + i] * D) ** 7 for i in range(16)]
    D = D ** 7
    N = m_e(N)
f = N[limb] - target * D
t1 = time.time()
deg = f.degree()
X = R([0, 1])
xp = X.pow_mod(P, f)
t_pow = time.time()
g = f.gcd(xp - X)
roots = g.roots() if g.degree() > 0 else []
t2 = time.time()

ok = 0
enc = 0
for (x, _) in roots:
    x = int(x)
    s = list(base); s[FREE] = x
    out = permute(s, rc=RC, rf_half=rf_half, rp=rp)
    if out[limb] == target:
        ok += 1
        enc += x < (1 << 56)
bound = 7 ** (2*rf_half) * 2 ** rp
import math
print(f"rf={rf_half}+{rf_half} rp={rp} bound_deg={bound} (2^{math.log2(bound):.2f}) deg={deg} "
      f"build={t1-t0:.2f}s powmod={t_pow-t1:.2f}s gcd_deg={g.degree()} roots={len(roots)} verified={ok} encodable={enc} solve={t2-t1:.2f}s")
