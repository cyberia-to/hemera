---
tags: cyber, cip
crystal-type: entity
crystal-domain: cyber
alias: Hemera reference, Hemera specification, Hemera spec, Hemera_Hash_Primitive_Reference
stake: 43936669831471920
---

# Hemera: A Permanent Hash Primitive for Planetary-Scale Collective Intelligence

| field    | value                          |
|----------|--------------------------------|
| version  | 1.0                            |
| status   | Decision Record                |
| authors  | mastercyb, Claude (Anthropic)  |
| date     | February 2026                  |

## Abstract

Hemera is the cryptographic hash primitive for [[cyber]], a knowledge graph for planetary-scale collective intelligence. It instantiates the Poseidon2 permutation over the [[Goldilocks field]] (p = 2^64 - 2^32 + 1) with state width t = 16, S-box degree d = 7, and 64 partial rounds (R_P = 64).

The construction provides 256-bit classical collision resistance and 170-bit quantum collision resistance. Algebraic degree 7^64 = 2^180 places the permutation far beyond any foreseeable attack capability. Every particle address in the network, every Merkle node in every proof tree, and every commitment in every STARK derives from the same permutation.

One function. One mode (sponge). 64 raw bytes output. These parameters are Hemera. If any parameter differs, it is not Hemera.

## Parameters

```
┌──────────────────────────────────────────────────────────┐
│  HEMERA — Complete Specification                         │
│                                                          │
│  Field:           p = 2⁶⁴ − 2³² + 1 (Goldilocks)       │
│  S-box:           d = 7  (x → x⁷, minimum for field)    │
│  State width:     t = 16                      = 2⁴       │
│  Full rounds:     R_F = 8  (4 + 4)            = 2³       │
│  Partial rounds:  R_P = 64                    = 2⁶       │
│  Rate:            r = 8  elements              = 2³       │
│  Input rate:      56 bytes/block (7 B/element) = 7 × 2³   │
│  Capacity:        c = 8  elements (64 bytes)   = 2³       │
│  Output:          8  elements (64 bytes)       = 2³       │
│                                                          │
│  Full round constants:    8 × 16 = 128        = 2⁷       │
│  Partial round constants: 64                  = 2⁶       │
│  Total constants:         192                 = 3 × 2⁶   │
│  Total rounds:            72                  = 9 × 2³   │
│                                                          │
│  Classical collision resistance:  256 bits     = 2⁸       │
│  Quantum collision resistance:   170 bits                │
│  Algebraic degree:               2¹⁸⁰                    │
│                                                          │
│  Every parameter that appears in code is a power of 2.   │
└──────────────────────────────────────────────────────────┘
```

## Specification pages

- [[field]] — Goldilocks prime field (canonical spec in [[aurum]])
- [[permutation]] — Poseidon2 round structure: S-box, linear layers, complete algorithm
- [[sponge]] — absorb/squeeze, padding, operational semantics
- [[capacity]] — structured capacity: flags, domain tags, counters, namespace bounds
- [[encoding]] — 7-byte canonical encoding, byte-to-field mapping
- [[tree]] — binary Merkle tree, `hash_node` construction
- [[constants]] — all 192 round constants (hex values)
- [[bootstrap]] — round constant self-generation via Hemera₀
- [[matrices]] — MDS and diagonal matrices for the linear layer
- [[api]] — public API surface: `hash`, `hash_node`, `absorb`, `squeeze`

## See also

- [[particle]] — particle addressing with Hemera
- [[cyberlink]] — edges referencing [[particles]] by Hemera hash
- [[cybergraph]] — the graph Hemera addresses
- [[nox]] — the VM where Hemera executes as a jet
- [[tri-kernel]] — probability engine consuming Hemera outputs
- [[cyber/proofs]] — STARK proof system built on Hemera
- [[cyber/bbg]] — graph database whose every tree structure uses hash_node
- [[WHIR]] — polynomial commitment scheme whose FRI trees use hash_node
- [[cyber/whitepaper]] — section 4 Hemera chapter
