---
tags: cyber, cip
crystal-type: entity
crystal-domain: cyber
alias: Hemera reference, Hemera specification, Hemera spec, Hemera_Hash_Primitive_Reference
stake: 43936669831471920
diffusion: 0.00010722364868599256
springs: 0.002067440114539792
heat: 0.001443745761651159
focus: 0.0009625930110351532
gravity: 0
density: 2.74
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

The construction provides 256-bit classical collision resistance and 170-bit quantum collision resistance. Algebraic degree 7⁸ × (p−2)¹⁶ ≈ 2^1046 places the permutation far beyond any foreseeable attack capability — 2^918 bits of margin over 128-bit security. Every [[particles|particle]] address in the network, every node in every proof tree, and every commitment in every [[STARK]] derives from the same permutation.

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
│  Classical collision resistance:  256 bits     = 2⁸       │
│  Quantum collision resistance:   170 bits                │
│  Algebraic degree:               2¹⁰⁴⁶                   │
│  Security margin:                2⁹¹⁸ over 128-bit       │
│                                                          │
│  MPC/FHE depth:                  40 (5.4× reduction)     │
│                                                          │
│  Every parameter that appears in code is a power of 2.   │
└──────────────────────────────────────────────────────────┘
```

## Design decisions

**x⁻¹ partial S-box.** field inversion replaces x⁷ in the 16 partial rounds. algebraic degree jumps from 7⁶⁴ ≈ 2¹⁸⁰ to 7⁸ × (p−2)¹⁶ ≈ 2¹⁰⁴⁶. partial rounds drop from 64 to 16 (4× fewer). constraints per permutation drop from ~1,152 to ~736 (36% reduction). MPC/FHE multiplicative depth drops from 216 to 40 (5.4× reduction). same wall-clock — fewer rounds but inversion costs more per round.

**32-byte output.** 4 elements instead of 8. 2× faster tree hashing (binary node fits in one rate block: 32+32=64 bytes ≤ 8×8=64 bytes). 2× less storage for roots and proofs. 256-bit collision resistance preserved (capacity is 8 elements = 64 bytes, unchanged). the output is a hash, not an encryption — 32 bytes is standard (SHA-256, Blake3, Keccak-256).

**16 partial rounds.** the minimum for security with x⁻¹ S-box. full rounds (8) provide diffusion across all 16 state elements. partial rounds (16) provide algebraic depth on element 0 only. the combination gives algebraic degree 2¹⁰⁴⁶ with only 24 total rounds.

## Specification pages

- [[field]] — Goldilocks prime field (canonical spec in [[nebu]])
- [[permutation]] — Poseidon2 round structure: S-box, linear layers, complete algorithm
- [[sponge]] — absorb/squeeze, padding, operational semantics
- [[capacity]] — structured capacity: flags, domain tags, counters, namespace bounds
- [[encoding]] — 7-byte canonical encoding, byte-to-field mapping
- [[tree]] — binary Merkle tree, `hash_node` construction
- [[constants]] — all 144 round constants (hex values)
- [[bootstrap]] — round constant self-generation via Hemera₀
- [[matrices]] — MDS and diagonal matrices for the linear layer
- [[api]] — public API surface: `hash`, `hash_node`, `absorb`, `squeeze`

## See also

- [[particles|particle]] — particle addressing with Hemera
- [[cyberlinks|cyberlink]] — edges referencing particles by Hemera hash
- [[cybergraph]] — the graph Hemera addresses
- [[nox]] — the VM where Hemera executes as a jet
- [[tri-kernel]] — probability engine consuming Hemera outputs
- [[cyber/proofs]] — [[STARK]] proof system built on Hemera
- [[BBG]] — authenticated state whose Lens commitment uses Hemera for binding
- [[Brakedown]] — Lens (polynomial commitment scheme), one Hemera call for binding hash
- [[cyber/whitepaper]] — section 4 Hemera chapter
