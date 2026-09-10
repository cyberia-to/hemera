---
tags: cyber, cip
crystal-type: process
crystal-domain: cyber
status: implemented
date: 2026-03-17
---
# inversion S-box — implemented candidate, security hypothesis open

Hemera implements x⁷ in eight full rounds and total inverse (0→0) in sixteen
partial rounds. Implementation status does not certify the parameter choice.

## hypothesis

A total inverse has a compact witness relation and good local differential
behavior. Investigate whether a mixed x⁷/inverse permutation can offer a useful
security/cost tradeoff. Neither increased full-permutation security nor the
64→16 round reduction follows from the exponent p−2.

## results of the first investigation

See [assessment](../research/inverse-sbox-assessment.md),
[executable analysis](../research/inverse_sbox.py) and
[results](../research/inverse-sbox-results.json).

- Correct witness relation: x(xy−1)=0 and y(xy−1)=0. Two cubic constraints,
  or three quadratic constraints with intermediate z=xy. The previous relation
  accepted y=0 for nonzero x; it was incorrect.
- Differential uniformity of total inverse over Goldilocks is exactly 4.
- Actual Rust inversion chain: 75 multiplications, exponent p−2, depth 71.
- Actual internal matrix: rank 16, irreducible characteristic polynomial;
  the first 16 coordinate-observability rows have full rank.
- The old 2^1046 degree / 2^918 security margin argument is withdrawn.
- Cheap verification does not establish one-multiplication inversion for
  MPC/FHE. The old 5.4× depth claim requires a separate protocol and measurement.

The original Poseidon authors discussed inverse S-boxes and warned about their
slow degree growth. This prior art is linked in the assessment. A 32-byte output
has an approximately 128-bit generic classical collision ceiling independently
of S-box choice; [[compact-output]] is a separate decision.

## before a parameter freeze

Analyze the exact hybrid against algebraic/CICO/preimage and subspace attacks,
including zero branches. Compare against power S-box configurations at matched
cost and security targets. Derive circuit costs from concrete backend wiring.
Obtain independent cryptographic review. No full-round attack or security proof
was produced by the preliminary algebraic checks.

Round constants are the current 144-element self-bootstrap from `cyber` in
`specs/bootstrap.md`; do not use the obsolete `cyber2` seed from this proposal's
initial text.
