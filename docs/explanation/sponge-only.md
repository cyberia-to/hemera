---
tags: cyber, cip
crystal-type: entity
crystal-domain: cyber
alias: "Hemera sponge only, why no compression mode"
---

# Why Sponge Only

Hemera uses sponge mode exclusively. Compression mode is rejected
for three independent reasons.

## Practical: ambiguity

Compression mode uses all 16 state elements as input (zero capacity).
Sponge mode reserves 8 as capacity. Two functions sharing one output
space means every downstream system must track which function produced
each address. That tracking is either a hidden type tag or an implicit
convention — a bug at planetary scale.

## Economic: irreversibility

The cost of sponge-only Merkle trees is 2x per internal node. Moore's
law eliminates any 2x decision in two years. Design ambiguity is
permanent. Accept 2x and buy performance through caching, incremental
updates, and parallelism — not a second mode.

## Mathematical: endofunctions

A sponge hash is an endofunction on the address space. Bytes in,
64 bytes out — valid input to the same function.
`Hemera(Hemera(x) || Hemera(y))` type-checks. Composition, chaining,
nesting — the algebra closes. A compression function has a different
type signature (128 bytes -> 64 bytes). Rejecting leaving the
category, not rejecting speed.
