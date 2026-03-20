---
tags: cyber, cip
crystal-type: process
crystal-domain: cyber
status: draft
date: 2026-03-20
---
# algebraic Fiat-Shamir

derive most Fiat-Shamir challenges algebraically from polynomial commitments. use hemera only for the initial seed. 8.7× fewer hemera calls in proof verification.

## current cost

zheng proof verification requires ~20 Fiat-Shamir challenges (one per sumcheck round + commitment rounds). each challenge: hemera(transcript) → squeeze.

```
current: 20 hemera calls × 736 constraints = 14,720 constraints
```

this is a significant fraction of the total recursive verification cost (~50,000 constraints with jets).

## the construction

the zheng-2 IOP (SuperSpartan + sumcheck) commits round polynomials via PCS (WHIR/Brakedown). the commitment is binding. an algebraic challenge can be derived from the commitment itself:

```
initial seed: hemera(instance)                    ~736 constraints (one-time)
challenge_0: seed                                  (derived from hemera)
challenge_i: poly_eval(commitment_i, seed_i)       ~50 constraints each

where seed_i = seed × i (or seed + i, any deterministic derivation)
```

## cost comparison

```
current Fiat-Shamir:    20 × 736 = 14,720 constraints
hybrid approach:        736 + 19 × 50 = 1,686 constraints
improvement:            8.7×
```

## security argument

algebraic Fiat-Shamir requires:
1. **binding commitment**: WHIR/Brakedown are computationally binding PCS — the prover cannot change the committed polynomial after seeing the challenge
2. **unpredictable evaluation point**: derived from the hemera-seeded random oracle — the prover cannot predict seed_i before committing
3. **algebraic independence**: evaluating a committed polynomial at an unpredictable point produces an unpredictable value (Schwartz-Zippel over Goldilocks: probability of collision ≤ degree / p)

the first challenge still uses hemera (bootstrapping the random oracle). subsequent challenges derive from polynomial evaluation — which is already what the IOP protocol verifies.

## hemera's evolving role

```
before:  hemera = Fiat-Shamir engine (20 calls per proof verification)
after:   hemera = trust anchor (1 call per proof verification)
         poly_eval = Fiat-Shamir engine (19 calls, 3.6× cheaper each)
```

hemera remains the cryptographic foundation. the bulk of challenge generation shifts to algebraic operations that are already part of the proof system.

## interaction with recursive verification

recursive verifier cost:

```
current:     ~50,000 constraints (with jets)
  of which:  ~14,720 hemera Fiat-Shamir
             ~33,000 Merkle verification
             ~2,280 other

with algebraic FS: ~36,966 constraints
  hemera:    736 (one call)
  poly_eval: 19 × 50 = 950
  Merkle:    ~33,000 (unchanged — see algebraic-extraction for this)
  other:     ~2,280

savings:     ~26% of recursive verifier cost
```

combined with algebraic extraction ([[algebraic-extraction]]) which eliminates Merkle verification: recursive verifier drops to ~4,000 constraints total.

## open questions

1. **formal security proof**: the hybrid Fiat-Shamir (hemera seed + algebraic derivation) needs a formal security reduction. the argument is standard but the concrete instantiation with WHIR/Brakedown commitments over Goldilocks needs verification
2. **round-by-round vs batch**: can all 19 algebraic challenges be derived in one batch polynomial evaluation? if so: 1 poly_eval call instead of 19
3. **transcript dependency**: each challenge depends on the previous commitment. the algebraic derivation must preserve this sequential dependency. seed_i must incorporate all prior commitments, not just the i-th

see [[inversion-sbox]] for hemera cost, [[algebraic-extraction]] for Merkle elimination, [[zheng-2]] for proof system architecture
