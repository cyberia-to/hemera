---
tags: hemera, eidos, verification, audit
crystal-type: report
crystal-domain: crypto
status: partial
---
# Eidos proof boundary

`proofs/` contains executable Eidos models and kernel-checked theorems.
The suite covers the full-round S-box multiplication schedule,
its witness equations, the four optimized M4 rows, zero minors in
the two 16-by-16 diffusion matrices, and the corrected inverse witness cases.

Acceptance requires actual proof terms: no `axiom`, `sorry`, unresolved
metavariables, or opaque constants. Algebraic laws, when needed, are explicit
theorem parameters. Eidos's public strict checker rechecks closed terms with the Eidos
Rust kernel and rejects deliberately false variants.

The models are manually related to `rs/src/field.rs` and `permutation.rs`.
They are not an extraction of Rust machine semantics. The connection is
documented and guarded by source fingerprints and implementation checks;
this does not prove Rust arithmetic, memory safety, or cryptographic security.

The trusted base is Eidos's Rust CIC kernel, reduction/substitution, the
inductive environment used by each proof, and Rust compilation/execution.
Eidos's nox/zheng certificate bridge is not used. These proofs do not claim
collision resistance, preimage resistance, or soundness of zheng.

## Obligations

| theorem | explicit premises | conclusion |
|---------|-------------------|------------|
| `traced_sbox_matches_plain` | any type and binary operation | both multiplication schedules coincide |
| `sbox_is_seventh_power` | associative multiplication | schedule equals seven right-associated factors |
| `full_round_witness_sound` | three witness/output multiplication equations | output equals the modeled S-box |
| `mat4_row{0,1,2,3}_matches_spec` | associative, commutative addition | optimized row equals the specified linear combination |
| `inverse_nonzero_product` | cancellation of multiplication by x, right identity, first cubic equation | xy=1 |
| `inverse_zero_output` | left/right zero multiplication, second cubic equation with x=0 | y=0 |
| `constant_minor_zero` | `sub x x = zero` | a constant 2-by-2 minor has zero determinant |

`swap_tail` is an auxiliary reassociation/commutation lemma. Twenty-one theorems are checked in total. `Binary.ei` adds two concrete
Goldilocks integer identities and two universal binary operation equations.
`Reduction.ei` adds six concrete reconstruction/range facts.
These do not prove primality or universal Rust modular arithmetic.

## Remaining work

- Establish the algebraic premises for the actual Goldilocks implementation,
  including noncanonical representatives, overflow and `reduce128`.
- Prove the exponent of the optimized inversion chain and the zero case.
- Formalize seven-byte encoding, canonical output decoding and sponge padding.
  Eidos now supports binary nonnegative arithmetic and concrete modular
  reduction certificates; universal modular/word semantics remain needed.
- Prove full permutation, streaming and tree refinements, then state the
  cryptographic assumptions separately from implementation correctness.

The inverse lemmas use expanded cubic equations x(xy)=x and y(xy)=y.
Instantiation over the actual Goldilocks field still requires its algebraic
laws and the equivalence to residual equations; the local Rust residual tests
and small-field exhaustive checks are reported separately. The experimental
structural commitment layer currently has Rust tests, not Eidos refinement
proofs. See [structural commitments](../specs/structural-commitments.md) for its obligations.
