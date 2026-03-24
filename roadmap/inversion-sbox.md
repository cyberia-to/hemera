---
tags: cyber, cip
crystal-type: process
crystal-domain: cyber
status: draft
date: 2026-03-17
diffusion: 0.00010722364868599256
springs: 0.0007414137239001096
heat: 0.0005436091679597342
focus: 0.0003847577751049711
gravity: 0
density: 0.29
---
# inversion S-box — x⁻¹ partial rounds with reduced count

replace x⁷ S-box in partial rounds with field inversion x⁻¹. reduce partial rounds from 64 to 16. the two changes are inseparable — x⁻¹ enables the round reduction.

## the insight

field inversion x⁻¹ = x^(p-2) over Goldilocks has algebraic degree p-2 ≈ 2^64 per application. x⁷ has degree 7 ≈ 2^2.8. each x⁻¹ partial round provides what x⁷ needs ~23 rounds to match.

```
x⁷:   64 partial rounds → degree 7^64 ≈ 2^180
x⁻¹:  16 partial rounds → degree (p-2)^16 ≈ 2^1024

with x⁻¹ total: 7^8 × (p-2)^16 ≈ 2^1046 (full + partial)
```

2^896 bits of margin over 2^128 security. 17× more margin in log-space than with x⁷.

## native cost

```
                    before (x⁷, 64 rounds)      after (x⁻¹, 16 rounds)
partial S-box:      64 × 3 = 192 muls            16 × 64 = 1,024 muls
partial MDS (M_I):  64 × 17 = 1,088 muls         16 × 17 = 272 muls
full rounds:        896 muls                      896 muls (unchanged)
total:              ~2,208 muls                   ~2,224 muls
throughput:         ~53 MB/s                      ~53 MB/s (unchanged)
```

21× more expensive S-box × 4× fewer rounds = same total.

## STARK constraints

```
                    before (x⁷, 64 rounds) after (x⁻¹, 16 rounds)
full S-boxes:       8 × 16 × 4 = 512       512 (unchanged, x⁷)
partial S-boxes:    64 × 4 = 256            16 × 2 = 32 (x⁻¹, verified as x×y=1)
MDS constraints:    ~256                    ~192
total:              ~1,152                  ~736
improvement:        —                       36% fewer constraints
```

x⁻¹ verification: prover provides y = x⁻¹. verifier checks:

```
x × y × (x × y - 1) = 0    AND    (1 - x × y) × y = 0
```

2 constraints vs 4 for x⁷ decomposition. handles zero: 0⁻¹ = 0.

## MPC depth

```
before (x⁷, 64 rounds): 216 sequential multiplications (8×3 full + 64×3 partial)
after  (x⁻¹, 16 rounds): 40 sequential multiplications (8×3 full + 16×1 partial)
improvement: 5.4×
```

at 10 ms network latency: 2.16 seconds → 0.40 seconds per hash.

## FHE noise

```
before (x⁷, 64 rounds): noise ∝ 2^216
after  (x⁻¹, 16 rounds): noise ∝ 2^40
improvement: 5.4× depth reduction
```

practical encrypted computation over hemera becomes feasible.

## fold steps (zheng)

```
before (x⁷, 64 rounds): 72 fold steps per hash × 30 ops = 2,160 ops
after  (x⁻¹, 16 rounds): 24 fold steps per hash × 30 ops = 720 ops
improvement: 3×
```

## the seven-and-inverse duality

x⁷ (minimal forward permutation) and x⁻¹ (inverse permutation) are algebraic complements. x⁷ provides fast native nonlinearity in full rounds (all 16 elements). x⁻¹ provides cheap verified nonlinearity in partial rounds (one element). both are forced by the Goldilocks prime structure.

## round constant generation

```
seed = [0x63, 0x79, 0x62, 0x65, 0x72, 0x32]    "cyber2"
procedure: Hemera_0 (all constants = 0) → absorb seed → squeeze 192 elements
only first 16 partial constants used
```

## open questions

1. **mixed S-box formal analysis**: x⁷ full + x⁻¹ partial interaction via MDS needs formal verification beyond the degree argument
2. **hardware acceleration**: GFP p2r pipeline needs field inversion support (maps to fma via square-and-multiply)
3. **bounty program**: Poseidon2 bounties don't cover hybrid S-box design

see [[compact-output]] for the output reduction, [[hemera]] for base specification