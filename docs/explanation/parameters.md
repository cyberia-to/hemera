---
tags: cyber, cip
crystal-type: entity
crystal-domain: cyber
alias: "Hemera parameter rationale, parameter decisions"
diffusion: 0.00010722364868599256
springs: 0.00007019991600688145
heat: 0.00003419142694206788
focus: 0.00008151008453347325
gravity: 0
density: 0
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

The ecosystem standard t=12 gives exactly 128-bit collision resistance with capacity 4 — zero margin.

BHT quantum collision search at cap=4 is 2^85, insufficient for a permanent system.

Security comparison:

| metric | cap=4 (t=12) | cap=8 (t=16) |
|---|---|---|
| classical collision | 2^128 | 2^256 |
| BHT quantum collision | 2^85 | 2^171 |
| classical preimage | 2^256 | 2^512 |
| Grover quantum preimage | 2^128 | 2^256 |

Throughput is identical: both have rate r=8 = 56 input bytes per permutation call.

## round counts: R_F=8, R_P=64

Full rounds (R_F=8): the wide trail strategy guarantees at least 8 active S-boxes across 4 full rounds. Differential probability per S-box is at most 6/2^64. Over 8 active S-boxes: (6/2^64)^8 ~ 2^-480.

Partial rounds (R_P=64): algebraic degree after 64 partial rounds is 7^64 ~ 2^180. An interpolation attack requires ~2^180 evaluations.

The 42 additional partial rounds beyond Plonky3's R_P=22 add only ~19% to total field multiplications. This is cheap margin for a permanent hash.

Context: the Ethereum Foundation bounty program has not produced attacks on Poseidon2 at standard round counts. Hemera's R_P=64 extends well beyond any published attack threshold.

## round structure: 8 + 64 = 72

The total 72 is not a power of 2. But the total never appears in code — it is a derived sum.

Loop bounds and array sizes are powers of 2:

- R_F = 8 (2^3)
- R_P = 64 (2^6)
- half-full = 4 (2^2)

R_P=64 was chosen over R_P=56 because the partial round constant array is a data structure. A 64-element array aligns to cache lines and simplifies indexing.

## computational elegance

Every parameter that appears as a loop bound, array size, or memory layout is a power of 2:

| parameter | value | power of 2 | code role |
|---|---|---|---|
| p (Goldilocks) | 2^64 - 2^32 + 1 | reduction via shifts | field arithmetic |
| t (state width) | 16 | 2^4 | array size, SIMD width |
| c (capacity) | 8 | 2^3 | security parameter |
| r (rate) | 8 | 2^3 | absorption loop bound |
| R_F (full rounds) | 8 | 2^3 | outer loop bound |
| R_P (partial rounds) | 64 | 2^6 | inner loop bound, constant array size |
| output (bytes) | 64 | 2^6 | output buffer size |
| element (bytes) | 8 | 2^3 | memory stride |

Only non-power-of-2 values: derived sums (72 total rounds, 192 total constants), input rate (56 = 7 x 8 bytes), and the S-box exponent d=7.

The Goldilocks prime forces 7 twice: as the S-box exponent (minimum invertible) and in the encoding rate (56 bytes = 7 field elements of 8 bytes each).

SIMD-aligned memory access, clean loop unrolling, cache-line alignment — all follow from the power-of-2 discipline.

The permutation loop structure:

```
for _ in 0..4:        // half-full rounds (power of 2)
    add_constants()
    sbox_full()        // 16 S-boxes (power of 2)
    mds()

for _ in 0..64:       // partial rounds (power of 2)
    add_constant()
    sbox_single()      // 1 S-box
    mds()

for _ in 0..4:        // half-full rounds (power of 2)
    add_constants()
    sbox_full()
    mds()
```