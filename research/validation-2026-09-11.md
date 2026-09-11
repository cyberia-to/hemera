# Validation — 2026-09-11

- Eidos `cargo test --quiet`: 161 tests pass, including binary carry tests,
  strict rejection controls and the doctest. No new dependencies.
- Hemera `python3 proofs/check.py`: 15 theorems checked, 11 negative controls
  rejected, source fingerprints unchanged, binary/M4/S-box bridges pass.
- Hemera `cargo test -p cyber-hemera --quiet`: 263 CPU unit/integration tests
  and one doctest pass. `cargo check -p cyber-hemera --no-default-features`
  passes. Existing unused-import warning in field.rs remains.
- `python3 research/partial_subspace.py`: exact constraints checked at every
  depth 1..16; report reproduces byte-for-byte. The Rust fifteen-round
  certificate test passes. Source SHA-256 values are in the report.
- Strict clippy remains blocked by the recorded baseline diagnostics:
  Eidos 13 across lib/test, Hemera lib 20. No diagnostics in the new binary
  arithmetic module. These checks are not reported as passing.

`params::COLLISION_BITS` changes from 256 to 128 to describe the approximate
generic ceiling of the existing 32-byte output. This informational public
constant is a metadata/API correction; permutation and digest bytes do not
change. Actual security is not established by that constant.

Limitations: no full-round attack or security proof, no Rust arithmetic
refinement, no measured proving-backend benchmark, no automatic adoption of
upstream Poseidon2. See [the decision](permutation-decision.md) for the exact
production disposition and remaining obligations.

## Follow-up after the production-finalization clarification

The earlier disposition is not a security conclusion; see `full-round-status.md`.

- Eidos: workspace tests plus four BNat tests pass (165 total, including the
  doctest); binary arithmetic also exercised in release mode. Strict CLI checks
  `examples/reduction.ei`. Strict clippy retains 13 baseline diagnostics.
- Hemera: 21 source theorems, 13 rejected negative controls, plus four closed
  arithmetic claims certifying two actual Rust products. The new full-round
  model/Rust digest test passes on all four vectors.
- Complete quadratic system: all witnesses satisfy all 1177 equations;
  every single-wire +1 perturbation is rejected for each of four witnesses.
  Exported constraints and witness are valid JSON. Report reproduces exactly.
- Universal manually translated `reduce128` model: Z3 4.15.4 returns UNSAT
  for negation of nine obligations; a wrong quotient formula has a SAT
  counterexample. Exported SMT-LIB replays UNSAT and reports reproduce exactly.

No full-round security proof, collision/preimage, or production parameter
finalization is claimed by these results.
