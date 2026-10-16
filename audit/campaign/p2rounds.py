"""Poseidon2 round-number inequalities (HorizenLabs/poseidon2 poseidon2_rust_params.sage,
sat_inequiv_alpha, incl. the eprint 2023/537 binomial term) re-implemented for
Goldilocks, alpha=7, t=16, M=128. Prints the cheapest admissible (R_F, R_P), the
minimal R_P per R_F, and each inequality's slack for given profiles."""
from math import log, ceil, floor, lgamma, inf
p = 2**64 - 2**32 + 1
FIELD_SIZE = ceil(log(p, 2))
def lbinom(n, k):
    return (lgamma(n + 1) - lgamma(k + 1) - lgamma(n - k + 1)) / log(2)
def terms(t, RF, RP, a=7, M=128):
    r1 = 6 if M <= (floor(log(p, 2) - (a - 1) / 2.0)) * (t + 1) else 10
    r2 = 1 + ceil(log(2, a) * min(M, FIELD_SIZE)) + ceil(log(t, a)) - RP
    r3 = log(2, a) * min(M, log(p, 2)) - RP
    r4 = t - 1 + log(2, a) * min(M / (t + 1), log(p, 2) / 2) - RP
    r5 = (t - 2 + M / (2 * log(a, 2)) - RP) / (t - 1)
    rt = floor(t / 3)
    over = (RF - 1) * t + RP + rt + rt * (RF / 2) + RP + a
    under = rt * (RF / 2) + RP + a
    gb4 = ceil(2 * lbinom(over, under))
    return dict(stat=r1, interp=r2, gb1=r3, gb2=r4, gb3=r5, gb4_bits=gb4)
def ok(t, RF, RP, M=128):
    d = terms(t, RF, RP, M=M)
    return RF >= max(ceil(d['stat']), ceil(d['interp']), ceil(d['gb1']), ceil(d['gb2']), ceil(d['gb3'])) and d['gb4_bits'] >= M
for t in (16, 24):
    best = None
    for RP in range(1, 200):
        for RF in range(4, 40, 2):
            if ok(t, RF, RP):
                c = t * RF + RP
                if best is None or c < best[0]:
                    best = (c, RF, RP)
                break
    c, RF, RP = best
    print(f"t={t}: minimal admissible RF={RF} RP={RP}; with the script's margin (+2 RF, x1.075 RP): RF={RF+2} RP={ceil(RP*1.075)}")
    for RF in (6, 8, 10, 12):
        mn = next((RP for RP in range(0, 400) if ok(t, RF, RP)), None)
        print(f"   RF={RF}: minimal RP = {mn}")
for t, RF, RP in [(16, 8, 22), (16, 12, 48), (24, 12, 48), (16, 8, 16)]:
    print(t, RF, RP, ok(t, RF, RP), {k: round(v, 2) for k, v in terms(t, RF, RP).items()})
