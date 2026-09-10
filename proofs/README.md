# Hemera proofs with Eidos

Run from `~/cyber/hemera` with sibling `../eidos` present:

```sh
python3 proofs/check.py
```

The command checks the reviewed Rust source fingerprints, builds the local
Eidos kernel and Hemera library, checks nine theorems, rejects seven negative
controls, and runs the separately labeled implementation/model checks.
No network, certificate service, nox or zheng is needed by the checker;
its two Rust dependencies are local path dependencies.

## Proofs

- [Sbox.ei](Sbox.ei): trace/plain equivalence, seventh-power schedule,
  and sufficiency of the S-box witness equations.
- [Matrix.ei](Matrix.ei): four M4 row equivalences, the supporting
  `swap_tail` lemma, and the determinant of a constant 2-by-2 minor.

The universal algebraic theorems use explicit function/law parameters.
There are no user axioms or admitted proofs. In particular, this suite
does **not** establish associativity of the Rust Goldilocks multiplication:
that is an outstanding premise when instantiating the abstract theorem.

## What is checked

`checker/` rejects `axiom`, `sorry`, imports, new inductive declarations,
opaque constants and unresolved metavariables. After elaboration it rechecks
every closed definition and proof in a fresh standard environment; theorem
types must be propositions. The allowed inductive fragment is Nat/Bool/Eq.

This stricter wrapper is intentional: the current Eidos frontend accepts
`sorry` and reports it as a theorem, and parts of its arithmetic surface
library are placeholders. The suite neither invokes nor trusts those paths.
It still trusts Eidos's Rust kernel and its standard inductive descriptors;
the wrapper is not an independent implementation or a kernel soundness proof.

Negative controls check rejection of `0=1`, a missing proof, an axiom,
an unresolved hole, a sixth-power S-box, an altered M4 coefficient, and
a missing output constraint. These checks must fail for the run to succeed.

## Connection to the implementation

The S-box definitions mirror `field.rs::pow7` and the multiplication
schedule of `permutation.rs::full_round_step`. M4 definitions are the
symbolic expansion of `field.rs::apply_mat4`, including its temporary values
and in-place write order. Fingerprints in `sources.json` force review when
either Rust file changes; changing the fingerprint alone establishes nothing.

The runner evaluates each Eidos M4 row on all four basis vectors and compares
all 16 coefficients with the actual Rust transform. It also compares Rust
S-box/trace behavior on 128 inputs, including noncanonical representatives.
These are bridge tests, not universal proofs of Rust behavior.

The actual internal 16×16 transform has a zero minor at rows `[0,1]`,
columns `[2,3]`; the external one has a zero minor at rows `[0,4]`, columns
`[8,12]`. The suite checks both. This corrects the all-minors-nonzero claim
in [matrices.md](../specs/matrices.md); a zero proper minor does not imply
the full matrix is singular.

Scope, assumptions and the next obligations: [formal-proofs.md](../specs/formal-proofs.md).
