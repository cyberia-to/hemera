---
tags: cyber, cip
crystal-type: entity
crystal-domain: cyber
alias: Hemera reference, Hemera specification, Hemera spec, Hemera_Hash_Primitive_Reference
---

# Hemera: A Permanent Hash Primitive for Planetary-Scale Collective Intelligence

| field    | value                          |
|----------|--------------------------------|
| version  | 2.0                            |
| status   | Decision Record                |
| authors  | mastercyb  |
| date     | March 2026                     |

## Abstract

Hemera is the cryptographic hash primitive for [[cyber]], a knowledge graph for planetary-scale collective intelligence. It instantiates the [[Poseidon2]] permutation over the [[Goldilocks field]] (p = 2^64 - 2^32 + 1) with state width t = 16, full-round S-box x⁷, partial-round S-box x⁻¹ (field inversion), and 16 partial rounds (R_P = 16).

The construction targets compact 32-byte identities. Its generic classical
collision ceiling is about 128 bits; security of the concrete hybrid
permutation remains under investigation. The old 2^1046 degree / 2^918 margin
argument is withdrawn. See `research/inverse-sbox-assessment.md`.

Hemera is the domain separation layer and trust anchor. Lens (Brakedown) handles bulk commitment — polynomial evaluation, batch openings, erasure coding. Hemera wraps Lens commitments with domain tags, providing identity binding and Fiat-Shamir seeding. Per execution, hemera is called ~3 times: (a) domain separation wrapper: hemera(Lens.commit(noun) ‖ tag) — one call per noun identity. (b) Fiat-Shamir seed — one call per proof. (c) Brakedown binding — one call per Lens commit (internal to Lens). The heavy work is polynomial arithmetic; hemera is the thin trust layer on top.

One function. One mode (sponge). 32 bytes output. ~736 constraints per permutation. These parameters are Hemera. If any parameter differs, it is not Hemera.

## Parameters

```
┌──────────────────────────────────────────────────────────┐
│  HEMERA — Complete Specification                         │
│                                                          │
│  Field:           p = 2⁶⁴ − 2³² + 1 (Goldilocks)       │
│  Full-round S-box: d = 7  (x → x⁷)                     │
│  Partial S-box:   x⁻¹    (field inversion)              │
│  State width:     t = 16                      = 2⁴       │
│  Full rounds:     R_F = 8  (4 + 4)            = 2³       │
│  Partial rounds:  R_P = 16                    = 2⁴       │
│  Rate:            r = 8  elements              = 2³       │
│  Input rate:      56 bytes/block (7 B/element) = 7 × 2³   │
│  Capacity:        c = 8  elements (64 bytes)   = 2³       │
│  Output:          4  elements (32 bytes)       = 2²       │
│                                                          │
│  Full round constants:    8 × 16 = 128        = 2⁷       │
│  Partial round constants: 16                  = 2⁴       │
│  Total constants:         144                 = 9 × 2⁴   │
│  Total rounds:            24                  = 3 × 2³   │
│                                                          │
│  Constraints per permutation: ~736                        │
│  Binary node:             1 permutation (32+32 ≤ rate)    │
│                                                          │
│  Classical collision ceiling:     128 bits     = 2⁸       │
│  Quantum collision security:     not certified                │
│  Algebraic attack margin:         not established                   │
│  Parameter status:               experimental       │
│                                                          │
│  MPC/FHE depth:                  protocol-dependent     │
│                                                          │
│  Every parameter that appears in code is a power of 2.   │
└──────────────────────────────────────────────────────────┘
```

## Design decisions

**x⁻¹ partial S-box.** Inversion admits two cubic or three quadratic local
constraints, including the zero case. This motivates investigation of the
cost/security tradeoff. Cheap witness verification does not imply cheap native,
MPC or FHE evaluation. The ~736 total is an unvalidated historical estimate.

**32-byte output.** Four canonical field elements. This halves digest storage
relative to eight elements, but sets the classical output collision ceiling
near 128 bits regardless of the capacity or S-box. Existing tree compression
loads two digests as eight field limbs; ordinary byte absorption uses a separate
seven-byte packing rule.

**16 partial rounds.** Implemented candidate, not a proven minimum. An analysis
of the exact mixed permutation is required before parameters are frozen.

**Structural identity.** The experimental format-independent commitment layer
provides position-independent blob IDs, ordered sequences and nominal records.
The `.cyb` adapter commits extracted sections. See [[structural-commitments]]
for encoding, opening verification, parser obligations and migration gates.

## Specification pages

- [[field]] — Goldilocks prime field (canonical spec in [[nebu]])
- [[permutation]] — Poseidon2 round structure: S-box, linear layers, complete algorithm
- [[sponge]] — absorb/squeeze, padding, operational semantics
- [[capacity]] — structured capacity: flags, domain tags, counters, namespace bounds
- [[encoding]] — 7-byte canonical encoding, byte-to-field mapping
- [[structural-commitments]] — experimental blobs, sequences, records and `.cyb` section adapter
- [[tree]] — binary Merkle tree, `hash_node` construction
- [[constants]] — all 144 round constants (hex values)
- [[bootstrap]] — round constant self-generation via Hemera₀
- [[matrices]] — MDS and diagonal matrices for the linear layer
- [[api]] — public API surface: `hash`, `hash_node`, `absorb`, `squeeze`

## See also

- [Eidos proof coverage report](../audit/formal-proofs.md) — checked theorems, explicit premises, validation boundary and remaining obligations

- [[files|file]] — the thing a particle identifies; its data is what Hemera hashes
- [[particles|particle]] — particle addressing with Hemera
- [[cyberlinks|cyberlink]] — edges referencing particles by Hemera hash
- [[cybergraph]] — the graph Hemera addresses
- [[nox]] — the VM where Hemera executes as a jet
- [[tri-kernel]] — probability engine consuming Hemera outputs
- [[cyber/proofs]] — [[zheng]] proof system built on Hemera
- [[BBG]] — authenticated state whose Lens commitment uses Hemera for binding
- [[Brakedown]] — Lens (polynomial commitment scheme), one Hemera call for binding hash
- [[cyber/whitepaper]] — section 4 Hemera chapter
