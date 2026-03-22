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
# constraint-free MDS — absorb linear layers into CCS wiring

MDS matrices (M_E, M_I) are public constants. encode them as CCS wiring rather than explicit constraints. 26% fewer constraints per hemera permutation.

## the insight

hemera constraint breakdown:

```
S-box constraints:    544 (degree 2, nonlinear — cannot be wired)
MDS constraints:      ~192 (degree 1, linear — CAN be wired)
total:                ~736
```

linear constraints of the form `a = 2b + 3c` can be absorbed into the CCS wiring matrix. the verifier doesn't check them as explicit constraints — the sumcheck implicitly verifies them through the committed trace polynomial.

## savings

```
with wired MDS:
  S-box constraints:    544 (unchanged)
  MDS constraints:      0 (absorbed into CCS wiring)
  total:                ~544
  improvement:          26% fewer explicit constraints
```

## mechanism

in standard CCS, the constraint system has matrices M₁, M₂, ... that multiply the witness vector. linear constraints like the MDS layer add rows to these matrices. wiring absorption means: instead of adding MDS as constraint rows, fold the MDS relations into the CCS matrix structure that connects round outputs to round inputs.

```
current CCS encoding:
  constraint row: state'[i] = d[i] × state[i] + Σ state[j]    (explicit)
  prover must satisfy this row independently

wired encoding:
  the round-to-round connection matrix already encodes:
    "output of round r, position i" connects to "input of round r+1, position i"
  extend this wiring to encode the MDS transform:
    "output of S-box round r" connects to "input of round r+1" via M_E or M_I weights
  no explicit constraint row needed — the wiring IS the constraint
```

## caveats

wiring absorption depends on the CCS encoding strategy in zheng. standard CCS libraries may not support weighted wiring natively. this is a zheng implementation detail — hemera's spec is unchanged.

the optimization is hemera-specific because hemera's MDS layers are the largest linear constraint block in the system. other operations (nox field arithmetic, LogUp) have smaller linear components.

## interaction with batched proving

when batching N hemera calls ([[batched-proving]]), the MDS wiring is shared across all N instances. the amortization is even better: N × 192 linear constraints eliminated → 0 explicit constraints for the linear component of all N calls.

## open questions

1. **zheng CCS support**: does zheng's CCS implementation support weighted wiring? if not, how much work to add it?
2. **verifier impact**: wired constraints are checked implicitly by sumcheck. does this change the verifier's work? (no — the sumcheck polynomial degree is determined by the highest-degree constraint, which is the S-box at degree 2. removing linear constraints doesn't change the verifier)

see [[inversion-sbox]] for S-box constraints, [[zheng-2]] for CCS encoding