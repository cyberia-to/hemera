---
tags: cyber, cip
crystal-type: process
crystal-domain: cyber
status: draft
date: 2026-03-20
---
# folded sponge — fold multi-block absorption into accumulator

for long inputs requiring K sponge blocks, fold each permutation into the running accumulator instead of proving K independent permutations. 7× savings for K=10.

## the observation

consecutive sponge absorption calls share state. the output of permutation N is the input of permutation N+1 (after absorbing the next block). this sequential dependency means K permutations are proved independently.

## the construction

with proof-carrying computation ([[proof-carrying]]) and HyperNova folding ([[folding-first]]):

```
single block:  input → 24 rounds → output           (1 perm, ~736 constraints)

K-block sponge (current):
  K independent permutations, each ~736 constraints
  total: K × 736 constraints

K-block sponge (folded):
  fold each permutation into accumulator: K × ~30 field ops
  one decider at end: ~736 constraints
  total: 736 + 30K field ops
```

## cost comparison

```
K = 1:     736 vs 766     (~1×, no savings)
K = 5:     3,680 vs 886   (4.2×)
K = 10:    7,360 vs 1,036  (7.1×)
K = 100:   73,600 vs 3,736 (19.7×)
K = 1000:  736,000 vs 30,736 (24×)
```

the savings grow with K. for large particles (content hashing) with many 56-byte blocks: substantial improvement.

## example: 4 KB particle

```
4096 bytes / 56 bytes per block = 74 blocks

current:  74 × 736 = 54,464 constraints
folded:   736 + 74 × 30 = 2,956 constraints
savings:  18.4×
```

## interaction with proof-carrying

in nox's proof-carrying execution, each hemera permutation during reduce() folds into the running accumulator automatically. the nox VM doesn't distinguish single-block from multi-block — every permutation is one fold step.

## interaction with batched proving

folded sponge and batched proving ([[batched-proving]]) address different scenarios:
- folded sponge: one long input (K blocks sequential)
- batched proving: N independent short inputs (parallel)

they compose: a block with N particles, each K blocks long:
- fold each particle's K blocks: K fold steps per particle
- batch the N deciders: one sumcheck for all N
- total: N × K × 30 fold ops + 736 + O(N) binding constraints

## open questions

1. **fold accumulator size**: each fold step adds ~30 field ops to the accumulator. for K=1000: 30,000 field ops in the accumulator. is the accumulator polynomial degree manageable?
2. **streaming compatibility**: hemera already supports streaming absorption (absorb block, permute, absorb next). folding integrates naturally — each permute step folds

see [[inversion-sbox]] for per-permutation cost, [[proof-carrying]] for fold mechanism, [[batched-proving]] for multi-instance optimization
