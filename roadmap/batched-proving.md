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
# batched hemera proving

when a block contains N hemera calls, prove them all with one batched sumcheck instead of N independent constraint sets. ~400× savings for N=1000.

## the observation

N independent hemera calls produce N × 24-round constraint sets. each set is structurally identical — same round constants, same MDS matrices, same S-box. only the input differs.

## the construction

express all N hemera calls as evaluations of a single multi-instance permutation polynomial:

```
P(x, instance) where:
  x encodes the round index (0..23)
  instance encodes which of the N calls (0..N-1)

constraint: P satisfies the hemera round relation at all (x, instance) pairs
proof: one sumcheck over the product domain {0..23} × {0..N-1}
```

## cost comparison

```
current:    N × 736 = 736N constraints (independent)
batched:    736 + O(N) field ops (shared structure, per-instance input binding)

N = 100:    73,600 → ~836    (88×)
N = 1000:   736,000 → ~1,736  (424×)
N = 10000:  7,360,000 → ~10,736 (686×)
```

the round constants and MDS matrices are shared across all N calls. the sumcheck amortizes structural verification. only input/output binding is per-instance.

## interaction with proof-carrying

with proof-carrying computation ([[proof-carrying]]), hemera calls during nox execution fold individually into the accumulator (~30 field ops per fold). batched proving applies to BLOCK-LEVEL aggregation: after folding, the decider verifies all hemera calls with one batch argument.

## open questions

1. **sumcheck domain**: the product domain {0..23} × {0..N-1} has size 24N. the sumcheck requires log₂(24N) rounds. at N=1000: ~15 rounds. each round: O(24N) prover work. total prover: O(24N × log(24N)). acceptable?
2. **different input sizes**: some hemera calls are single-block (short input), some are multi-block (long input). batching requires uniform structure. option: batch by block count (all 1-block calls together, all 2-block calls together)
3. **cross-block dependency**: Fiat-Shamir hemera calls depend on previous transcript. they cannot be batched with independent content hashes. separate batch for Fiat-Shamir vs content addressing

see [[inversion-sbox]] for per-call cost, [[proof-carrying]] for fold integration