# Verification foundation validation — 2026-09-10

Implemented: an additive experimental structural identity layer, an adapter for
extracted `.cyb` sections, corrected inverse witness residuals and two conditional
Eidos lemmas. Hash parameters and existing particle address APIs are unchanged.

Checks run:

- Eidos `cargo test --workspace --quiet`: 158 tests passed.
- Hemera `cargo test -p cyber-hemera --lib --tests --quiet`: 262 tests passed,
  including existing vectors, CDC tests, inverse residual tests, sequence
  accumulator carry/overflow tests and opening tampering tests.
- New inverse and structural integration tests also passed in release mode.
- `cargo check -p cyber-hemera --no-default-features`: passed.
- `python3 proofs/check.py`: 11 theorems and 9 rejected negative controls;
  separately labeled implementation/model comparisons passed.
- `python3 research/inverse_sbox.py`: reproduced the saved JSON exactly.
- `cargo run -p cyber-hemera --example structural_identity`: passed. The first
  and second container roots differ; their section ID remains
  `e22a6bf359bdb84db70cd3b403da2d77f2abfb02a9a94eb79bfd2d0ded93f4bb`.
  Chunk 1 opens with two siblings. These are experimental example outputs,
  not a protocol freeze.

Strict clippy is not green in the existing repositories. Isolated exports of
the pre-change commits reproduced 16 Eidos diagnostics (all targets) and 20
Hemera diagnostics (library). The final runs have 13 and 20 respectively;
no remaining diagnostic points to the newly added modules. Existing lints
include large error variants, loop/style issues and manual div_ceil. Hemera's
existing field test also reports an unused import. This work does not claim
a clean whole-repository lint baseline.

The documented rsc executable was absent at both the documented ~/git/rs path
and the sibling ~/cyber/rs path. Rust/no_std builds were checked; rs-edition
compiler conformance was not executed. New structural modules avoid internal
heap allocation without lint suppressions. The caller owns input buffers and
the `.cyb` name scratch slice.

Not established: formal refinement of the structural Rust modules; verified
`.cyb` parser extraction; complete binary arithmetic in Eidos; kernel soundness;
full-round cryptographic security; GPU equivalence for these new APIs; mission
qualification. Next obligations are listed in the structural spec, Eidos strict
spec and inverse assessment.
