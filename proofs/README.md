# Hemera proofs with Eidos

Run from `~/cyber/hemera` with sibling `../eidos` present:

```sh
python3 proofs/check.py
```

The command checks the reviewed Rust source fingerprints, builds the local
Eidos kernel and Hemera library, checks eleven theorems, rejects nine negative
controls, and runs the separately labeled implementation/model checks.
No network, certificate service, nox or zheng is needed by the checker;
its two Rust dependencies are local path dependencies.

## Proofs

- [Inverse.ei](Inverse.ei): conditional sufficiency of the corrected inverse
  equations for the zero and cancellable-input cases.
- [Sbox.ei](Sbox.ei): trace/plain equivalence, seventh-power schedule,
  and sufficiency of the S-box witness equations.
- [Matrix.ei](Matrix.ei): four M4 row equivalences, the supporting
  `swap_tail` lemma, and the determinant of a constant 2-by-2 minor.

The universal algebraic theorems use explicit function/law parameters.
There are no user axioms or admitted proofs. In particular, this suite
does **not** establish associativity of the Rust Goldilocks multiplication:
that is an outstanding premise when instantiating the abstract theorem.

## What is checked

`checker/` calls Eidos's public `check_strict_source` API. The strict checker
rejects axioms, sorry, opaque constants, holes, user inductives and declaration
shadowing; it rechecks closed terms against a fresh Nat/Bool/Eq environment.
Eidos also provides `check --strict file.ei`, which checks transitive imports
with cycle detection. See [Eidos strict policy](../../eidos/specs/strict-checking.md).

The restricted prelude excludes unsupported arithmetic placeholders. Eidos now
rejects `sorry` in ordinary checks too. The Rust kernel and fixed inductive
descriptors remain trusted; this is not an independent kernel implementation.

Negative controls check rejection of `0=1`, a missing proof, an axiom,
an unresolved hole, a sixth-power S-box, an altered M4 coefficient, and
a missing output constraint, and each missing inverse constraint. These checks must fail for the run to succeed.

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
