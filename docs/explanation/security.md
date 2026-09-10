---
tags: cyber, cip
crystal-type: entity
crystal-domain: cyber
alias: "Hemera security, ecosystem context"
---

# security status

Hemera currently uses a mixed x⁷ / total-inverse permutation over Goldilocks,
width 16, RF=8 and RP=16. This is an experimental modification of Poseidon2.
Security results and deployment experience for other permutations do not
establish security of these parameters.

The [inverse S-box assessment](../../research/inverse-sbox-assessment.md)
contains reproducible algebraic checks and primary references. It distinguishes
local S-box properties, circuit soundness, linear-layer checks and full-round
cryptanalysis. No full-round security certification is claimed.

The 32-byte output imposes a generic classical collision ceiling near 128 bits.
Capacity and digest length have different roles; an inverse S-box does not
remove the output birthday bound.

The structural identity layer adds stable content IDs, ordered commitments and
selective openings. Its binding arguments depend on the underlying hash and
on correct format extraction. See [structural commitments](../../specs/structural-commitments.md).

For implementation proof coverage and remaining assumptions, see
[formal proofs](../../specs/formal-proofs.md). Kernel-checked algebraic lemmas
and Rust tests do not by themselves certify cryptographic strength, constant-time
behavior or a mission-critical deployed system.
