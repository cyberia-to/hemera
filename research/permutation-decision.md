# Permutation decision — 2026-09-11

**Superseded as an answer to the production-finalization request.** The user
requires security to be established or a concrete insecurity result, rather
than a recommendation based on uncertainty. Neither has been achieved. The
cost comparison below remains evidence, not a production parameter decision.
See [the continuing full-round investigation](full-round-status.md). No live
permutation constants, round counts, or hash outputs change in this work.

The original justification (huge polynomial degree implies superior security
and permits sixteen partial rounds) fails. The smaller local inverse constraint
system is real, but it buys too little in the presently measured cost model
to justify treating an unanalysed permutation as mission-ready. Increasing RP
arbitrarily would not supply the missing analysis either.

## Matched arithmetic cost

The [pinned Plonky3 implementation](https://github.com/Plonky3/Plonky3/blob/4faefa94a1b58c5693e7f1151034e23cd1d4cec6/goldilocks/src/poseidon2.rs)
uses width 16, RF=8, RP=22, exponent 7. Its official constants and matrices
are part of that profile: merely putting x^7 and 22 rounds into Hemera with
custom constants would not reproduce it or inherit its analysis.

`python3 research/partial_subspace.py` reproduces this operation model from
the actual inverse chain and the specified quadratic encodings:

| Profile | Native nonlinear multiplications | Quadratic nonlinear constraints |
|---|---:|---:|
| Hemera inverse, RF8/RP16 | 1712 | 560 |
| x^7 RF8/RP16, cost control only | 576 | 576 |
| x^7 RF8/RP22, reference round count | 600 | 600 |

The inverse saves **6.67%** of these constraints against RP22, while using
**2.85 times** the nonlinear multiplications in the current native chain.
At equal RP16 it saves only 2.78% of constraints. With the *current Hemera*
16-multiplication internal diagonal, the respective native totals are 1968,
832 and 952. These totals are a controlled layer-cost comparison, not a
benchmark of the optimized Plonky3 implementation. Squares count as multiplications;
additions, witness generation, circuit layout and proving time are excluded.
Different gates/proof backends can change the tradeoff. No equal-security claim
is implicit in comparing these rows.

## A concrete partial-layer restriction

The script reads the real M_I and all sixteen internal constants. It constructs
affine input spaces on which every active S-box input equals 1. Inversion and
x^7 both fix 1, so the corresponding restricted transitions are affine.

Write the input as v and the linearized state before round j as A_j v+b_j.
Impose `row0(A_j) v = 1-b_j[0]-rc_j`. Replace the S-box by the identity on
that constrained coordinate, then propagate A and b through M_I. Gaussian
elimination gives independent constraints for k=1,...,16, hence dimension
16-k. The report supplies a base point and nullspace basis for k=15 and 16;
the script verifies all linear equations over the actual Goldilocks prime.

In particular **15 partial rounds have a p-element affine line of inputs
whose outputs are affine in the line parameter**. Sixteen constraints leave
one point in this particular construction. That is not a security threshold:
it neither proves RP16 sufficient nor rules out other restrictions or attacks.

`cargo test -p cyber-hemera --test partial_subspace` evaluates the fifteen-round
certificate through the actual Rust field and linear-layer implementation on
five parameters, with a perturbed-input negative control. The linear equations
establish the model's whole family; Rust sampling is only an implementation
bridge, not a universal refinement proof. Regenerate the fixture with
`python3 research/partial_subspace.py --write-rust-fixture`.

This is **not a full-round collision or preimage attack**. The family starts at
the partial-layer boundary with sixteen free field coordinates. It does not
respect the sponge's fixed capacity or byte encoding; traversing four initial
and four final nonlinear layers introduces further constraints. A full attack
must satisfy those constraints and account for solving cost. The same basic
construction applies to x^7, so it does not distinguish the two S-box choices.

The [2026/1692 preprint abstract](https://eprint.iacr.org/2026/1692) reports
related subspace restrictions and reduced-round CICO results for Poseidon over
KoalaBear. Only the abstract was accessible here (PDF download returned 403).
We do not claim to reproduce its algorithm or transfer its results to Hemera
or Poseidon2. Reviewing its full model is an outstanding obligation when
assessing the alternative, not grounds to call that alternative broken.

## What would reopen inverse-16 for production

1. Specify the exact security games and target: approximately 128 classical
   collision bits is the generic ceiling for the existing 32-byte digest,
   not 256. Fix preimage, multi-target and proof-system requirements separately.
2. Analyse the exact hybrid, including all zero-inversion branches, subspace
   restrictions across full rounds, differential trails and algebraic CICO
   systems. Give explicit attack bounds and round margins rather than equating
   polynomial degree with attack complexity.
3. Reproduce candidate attacks and assess scaling; obtain independent review
   of the full configuration. A solver timeout is not a lower bound.
4. Benchmark real proof and native backends against the pinned alternative.
   Establish a meaningful measured advantage at the required security margin.

These obligations are not completed by the present scripts or Eidos proofs.
Formal functional correctness cannot certify the absence of cheaper attacks.

## Next implementation boundary

Build the pinned all-x^7 profile as an explicit experimental profile with
upstream known-answer tests; compare sponge/commitment workload and proof cost.
Do not transplant only its RP value. Freeze a production profile only after
reviewing its analysis against the intended security games and newer attacks.
Any eventual replacement needs versioned identities and a root migration.
The structural tree and `.cyb` semantic adapter remain useful independently
of which permutation is chosen.

For Eidos the immediate next dependency is binary zero/word semantics and
modular reduction, followed by a universal `reduce128` refinement theorem.
This slice adds real kernel-reduced binary positives and Goldilocks integer
identities; it does not pretend those identities prove the field implementation.
