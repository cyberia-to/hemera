---
tags: cyber, cip
crystal-type: process
crystal-domain: cyber
status: draft
date: 2026-03-20
diffusion: 0.00010722364868599256
springs: 0.00007019991600688145
heat: 0.00003419142694206788
focus: 0.00008151008453347325
gravity: 0
density: 0
---
# partial-round collapse

states [1..15] evolve linearly during all 16 partial rounds. precompute the composed linear map. break the sequential dependency for 15 of 16 state elements. ~4× prover wall-clock speedup.

## the observation

in partial rounds, only state[0] gets the nonlinear x⁻¹ S-box. states [1..15] only get linear mixing via M_I:

```
partial round i:
  state[0] = (state[0] + RC[i])⁻¹       ← nonlinear (sequential)
  state = M_I × state                    ← linear for [1..15]
```

the linear evolution of states [1..15] across all 16 rounds can be expressed as:

```
state_final[1..15] = L₁₆ × state_initial[1..15] + f(state[0] values at each round)

where:
  L₁₆ = M_I^16 restricted to indices [1..15]   (15×15 constant matrix)
  f = contribution of state[0] at each round     (depends on x⁻¹ values)
```

## prover advantage

```
current: 16 sequential rounds, each touching all 16 elements
  prover must evaluate M_I × state at every round
  16 × 16 multiplications per round = 4,352 total field muls

collapsed: precompute L₁₆ (constant matrix), apply once
  L₁₆ is a 15×15 matrix — precomputed at compile time
  ONE matrix-vector multiply: 15 × 15 = 225 field muls
  + 16 sequential x⁻¹ on state[0]: 16 × 64 = 1,024 field muls
  + 16 injection steps: 16 × 15 = 240 field muls
  total: ~1,489 field muls

prover speedup: 4,352 / 1,489 ≈ 2.9×
```

the constraint count is similar (linear constraints in both encodings), but prover wall-clock drops because the sequential dependency is broken for 15 of 16 elements.

## SIMD optimization

the L₁₆ matrix-vector multiply is embarrassingly parallel. 15 independent dot products of length 15. maps perfectly to SIMD (4 lanes) or GPU (15 parallel threads).

```
sequential: 225 field muls, 15 sequential accumulations
SIMD (4-wide): ~60 field muls wall-clock
GPU: ~15 field muls wall-clock
```

## interaction with batched proving

partial-round collapse composes with batched proving ([[batched-proving]]): for N hemera calls, the L₁₆ application is N independent matrix-vector multiplies. all N can run in parallel. the sequential bottleneck is only the N × 16 x⁻¹ applications on state[0].

## open questions

1. **L₁₆ numerical stability**: the 15×15 matrix is a 16th power of M_I restricted to [1..15]. need to verify that the entries remain in canonical Goldilocks range (they will, since M_I entries are small)
2. **constraint encoding**: does the collapsed form produce different constraint structure than round-by-round? the verifier still checks the same relation, but the CCS layout may need adaptation

see [[inversion-sbox]] for the x⁻¹ S-box, [[batched-proving]] for multi-instance optimization