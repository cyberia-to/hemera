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

Eight Goldilocks capacity elements provide roughly 512 bits of internal
state capacity. Four output elements provide roughly 256 output bits, so the
generic classical collision ceiling of the actual digest is about 128 bits.
A larger capacity does not increase that output birthday bound. The rate is
eight elements / 56 input bytes; equal rates alone do not imply equal throughput
for different state widths or permutations.

## round counts: R_F=8, R_P=16

Eight full rounds use x⁷; sixteen partial rounds use total inverse (0→0).
This is the current experimental configuration. Its full-round security and
adequacy of the round count require analysis of the exact mixed construction.
The former multiplication of p−2 degrees into a 2^918 security margin was
incorrect as a security argument and is withdrawn.

The [inverse S-box assessment](../../research/inverse-sbox-assessment.md)
records the corrected witness relation, exact local differential bound,
actual addition-chain cost, matrix checks and remaining cryptanalysis.

## round structure: 8 + 16 = 24

total 24 = 3 × 2³. every component is a power of 2 (R_F=8=2³, R_P=16=2⁴). the round structure: 4 initial full rounds + 16 partial rounds + 4 terminal full rounds.

Loop bounds and array sizes are powers of 2:

- R_F = 8 (2^3)
- R_P = 16 (2^4)
- half-full = 4 (2^2)

R_P=16 is an implemented choice, not a proved minimum or a certified security margin.

## computational elegance

Every parameter that appears as a loop bound, array size, or memory layout is a power of 2:

| parameter | value | power of 2 | code role |
|---|---|---|---|
| p (Goldilocks) | 2^64 - 2^32 + 1 | reduction via shifts | field arithmetic |
| t (state width) | 16 | 2^4 | array size, SIMD width |
| c (capacity) | 8 | 2^3 | security parameter |
| r (rate) | 8 | 2^3 | absorption loop bound |
| R_F (full rounds) | 8 | 2^3 | outer loop bound |
| R_P (partial rounds) | 16 | 2^4 | inner loop bound, constant array size |
| output (bytes) | 32 | 2^5 | output buffer size |
| element (bytes) | 8 | 2^3 | memory stride |

Only non-power-of-2 values: derived sums (24 total rounds, 144 total constants), input rate (56 = 7 x 8 bytes), and the S-box exponent d=7.

The Goldilocks prime forces 7 twice: as the S-box exponent (minimum invertible) and in the encoding rate (56 bytes = 7 field elements of 8 bytes each).

SIMD-aligned memory access, clean loop unrolling, cache-line alignment — all follow from the power-of-2 discipline.

The permutation loop structure:

```
for _ in 0..4:        // half-full rounds (power of 2)
    add_constants()
    sbox_full()        // 16 S-boxes (power of 2)
    mds()

for _ in 0..16:       // partial rounds (power of 2)
    add_constant()
    sbox_single()      // 1 S-box
    mds()

for _ in 0..4:        // half-full rounds (power of 2)
    add_constants()
    sbox_full()
    mds()
```