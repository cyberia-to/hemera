---
tags: cyber, cip
crystal-type: entity
crystal-domain: cyber
alias: "Hemera parameter rationale, parameter decisions"
---

# parameter decisions

## field: Goldilocks

Why not 31-bit fields: capacity=8 at 31 bits yields only 124 bits collision resistance.

Why not 254-bit: multiprecision costs ~10x more than native 64-bit.

Why Goldilocks (p = 2^64 - 2^32 + 1):

- native CPU width — single 64-bit register per element
- fast reduction — subtract-and-shift, no division
- large NTT domain — multiplicative group of order 2^32
- curve independence — no coupling to any elliptic curve
- 8-byte elements — clean alignment, no padding

## S-box: d=7

The S-box exponent must be a bijection over the field: gcd(d, p-1) = 1.

- d=3: gcd(3, p-1) = 3. Not invertible.
- d=5: gcd(5, p-1) = 5. Not invertible.
- d=7: gcd(7, p-1) = 1. Invertible.

d=7 is the minimum invertible exponent. Multiplicative depth is 3 (computed as x -> x^2 -> x^4 -> x^3 * x^4 = x^7 with squarings and one multiply).

## state width: t=16, r=8, c=8

The width-12 Goldilocks instances common in the ecosystem use capacity 4 (256 bits). Hemera uses capacity 8 (512 bits): the generic sponge bounds sit at 2^256 classical and ≈ 2^170 quantum, above every digest length Hemera offers, so the digest — not the capacity — is what binds.

| metric | 32-byte digest (Hemera) | 48-byte digest | 64-byte digest |
|---|---|---|---|
| classical collision | 2^128 | 2^192 | 2^256 |
| BHT quantum collision | 2^85.3 | 2^128 | 2^170 (capacity-bound) |
| classical preimage | 2^256 | 2^256 (capacity) | 2^256 (capacity) |
| Grover quantum preimage | 2^128 | 2^128 | 2^128 |

the 32-byte digest caps collision at 2^128 classical and preimage at p⁴ ≈ 2^256; a capacity of 8 elements does not raise either. Throughput is set by the rate: r = 8 = 56 input bytes per permutation call.

## S-box everywhere: x⁷

Hemera 0.4 applies x⁷ in every round, full and partial — the Poseidon2 reference S-box for Goldilocks, the one every published bound, round-number inequality and bounty instance is written for.

Hemera 0.3 used x⁻¹ (field inversion) in its partial rounds and argued from degree: p − 2 per partial round, "7⁸ × (p−2)¹⁶ ≈ 2^1046". That number is the unreduced composition degree, and an attacker never meets it. Carried as a rational function of one input, an inverse round only doubles the degree, so the 0.3 permutation has a one-variable CICO degree of 7⁸ · 2¹⁶ ≈ 2^38.46 — below p — and a reduced instance with all sixteen inverse rounds was solved by rational interpolation in 92 seconds (`audit/campaign.md` §3.2). The degree argument is withdrawn; the 0.3 profile is kept only as a record.

## round counts: R_F = 12, R_P = 48

The counts follow a rule set against the published attacks (`audit/campaign.md` §3.3):

- **full rounds**: each outer layer keeps at least three times the full rounds a published attack crosses or skips. The round-skipping technique of Bariant et al. (ToSC 2022) skips two; ePrint 2026/1692 crosses one. 3 × 2 = 6 per side → R_F = 12.
- **partial rounds**: at least 1.5 times the longest published subspace trail through the internal layer, ≈ 30 nonlinear rounds at t = 16 (ePrint 2026/1792). 1.5 × 30 = 45 → R_P = 48.

The Poseidon2 round-number inequalities (`audit/campaign/p2rounds.py`, the Horizen Labs script re-implemented) need only R_P ≥ 14 at R_F = 12; the reference instance for Goldilocks t = 16 is (8, 22). Hemera pays 1.7× the reference rounds for margin that is fixed at genesis: rounds are the one parameter that cannot be raised later without rehashing the graph.

Margins against each attack line are in [[permutation]] § round numbers and margins. The algebraic margin is measured by the cryptanalysis campaign, not claimed.

## round structure: 6 + 48 + 6 = 60

```
M_E                   // initial linear layer
for _ in 0..6:        // initial full rounds
    add_constants()   // 16 constants
    sbox_full()       // 16 × x⁷
    M_E()

for _ in 0..48:       // partial rounds
    add_constant()    // 1 constant
    sbox_single()     // 1 × x⁷ on state[0]
    M_I()

for _ in 0..6:        // terminal full rounds
    add_constants()
    sbox_full()
    M_E()
```

240 constants (192 external, 48 internal), 240 S-boxes, 1,728 multiplications per permutation (768 in full-round S-boxes, 192 in partial-round S-boxes, 768 in the internal layer). The power-of-two discipline of earlier versions (R_F = 8, R_P = 16) yields to the round rule: 12 and 48 are what the attacks ask for.

## cost

| | hemera 0.3 | hemera 0.4 |
|---|---|---|
| S-boxes | 128 x⁷ + 16 x⁻¹ | 240 x⁷ |
| constraints (hemera accounting: 4 per x⁷, 2 per x⁻¹, ~8 per linear layer) | ~736 | ~1,440 |
| native multiplications | ~2,224 (inverse chains) | 1,728 |
| one permutation, M4 Max | 2.97 µs | 1.35 µs |
| batched, per permutation, M4 Max | 0.73 µs (one inversion shared across 16 lanes) | 1.23 µs |

measurements: `audit/profile-0.4-2026-10.md`.
