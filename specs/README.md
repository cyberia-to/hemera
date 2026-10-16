---
tags: cyber, cip
crystal-type: entity
crystal-domain: cyber
alias: Hemera reference, Hemera specification, Hemera spec, Hemera_Hash_Primitive_Reference
---

# Hemera: A Permanent Hash Primitive for Planetary-Scale Collective Intelligence

| field    | value                          |
|----------|--------------------------------|
| version  | 0.4 (profile)                  |
| status   | Decision Record                |
| authors  | mastercyb  |
| date     | October 2026                   |

## Abstract

Hemera is the cryptographic hash primitive for [[cyber]], a knowledge graph for planetary-scale collective intelligence. It instantiates the [[Poseidon2]] permutation over the [[Goldilocks field]] (p = 2^64 - 2^32 + 1) with state width t = 16, the S-box x⁷ in every round, 12 full rounds (6 + 6) and 48 partial rounds, and the Poseidon2 reference linear layers for this width and field.

The 8-element capacity (512 bits) bounds generic attacks on the sponge far above any output length. Collision resistance is set by the 32-byte (4-element) output: 128-bit classical (birthday on p⁴ ≈ 2^256 digests) and ~85-bit quantum (BHT) — see the hash row of the [[zheng]] soundness ledger (`zheng/specs/soundness.md`) and the digest-length table below. The algebraic margin is measured by the public cryptanalysis campaign (`audit/campaign.md`), not claimed by a degree bound. Every [[particles|particle]] address in the network, every node in every proof tree, and every commitment in every [[zheng]] proof derives from the same permutation.

Hemera is the domain separation layer and trust anchor, and the only hash in the proof system. [[lens]] commits polynomials as Reed–Solomon codewords (WHIR, TensorRs); hemera builds the Merkle trees of those commitments, the authentication paths of every opening, and the Fiat–Shamir transcript of every [[zheng]] proof, and wraps commitments with domain tags for identity binding. The number of hemera calls therefore scales with the opening's queries, not with a fixed count per execution. Hemera speed is a leading term of zheng verify time.

One permutation. One sponge. 32 bytes output. 240 S-boxes per permutation (~1,440 constraints in hemera's accounting: 960 for S-boxes, ~480 for the linear layers). These parameters are Hemera. If any parameter differs, it is not Hemera.

## Parameters

```
┌──────────────────────────────────────────────────────────┐
│  HEMERA 0.4 — Complete Specification                     │
│                                                          │
│  Field:           p = 2⁶⁴ − 2³² + 1 (Goldilocks)         │
│  S-box:           x⁷, every round                        │
│  State width:     t = 16                                 │
│  Full rounds:     R_F = 12  (6 + 6)                      │
│  Partial rounds:  R_P = 48                               │
│  External layer:  circ(2·M4, M4, M4, M4)                 │
│                   M4 = [5 7 1 3; 4 6 1 1; 1 3 5 7; 1 1 4 6] │
│  Internal layer:  1 + diag(d), d = MAT_DIAG16_M_1        │
│  Rate:            r = 8  elements                        │
│  Input rate:      56 bytes/block (7 B/element)           │
│  Capacity:        c = 8  elements (64 bytes)             │
│  Output:          4  elements (32 bytes)                 │
│                                                          │
│  Full round constants:    12 × 16 = 192                  │
│  Partial round constants: 48                             │
│  Total constants:         240  (seed "cyber", counter 0) │
│  Total rounds:            60                             │
│                                                          │
│  Multiplications per permutation: 1,728                  │
│  Binary node:             1 permutation (32+32 ≤ rate)   │
│                                                          │
│  Classical collision resistance:  128 bits               │
│  Quantum collision resistance:   ~85 bits (BHT)          │
│  Algebraic margin: measured by the campaign              │
└──────────────────────────────────────────────────────────┘
```

## Digest length

The digest, not the rounds, sets the generic bounds. The capacity-side bounds of the sponge are 2^256 classical and ≈ 2^170 quantum (BHT) at c = 512 bits; a shorter digest binds below them.

| digest | classical collision | quantum collision (BHT) | classical preimage | where |
|---|---|---|---|---|
| 4 elements, 32 B (Hemera 0.4) | 2^128 | 2^85.3 | 2^256 | `hash`, `hash_node`, every identity and tree node |
| 6 elements, 48 B | 2^192 | 2^128 | 2^256 (capacity) | 48 bytes of the XOF (`finalize_xof`); not an identity format |
| 8 elements, 64 B | 2^256 | 2^170 (capacity-bound) | 2^256 (capacity) | 64 bytes of the XOF; a 64-byte identity would need t = 24 to keep one permutation per tree node (hemera#15) |

Hemera 0.4 keeps the 32-byte digest: the permutation changes, the identity format does not. A post-quantum collision requirement of 128 bits is met at 48 bytes; that is a separate decision (it changes the particle size and the tree node layout) and changes no row of the permutation's margin table except the collision row.

## Design decisions

**x⁷ in every round.** The Poseidon2 reference S-box over Goldilocks; every published bound, every round-number inequality and every bounty instance is written for it. Hemera 0.3 used x⁻¹ in its 16 partial rounds on a degree argument (degree p − 2 per round) that does not survive cryptanalysis: carried as a rational function the inverse doubles the degree per round, the 0.3 permutation has a one-variable CICO degree of 7⁸ · 2¹⁶ ≈ 2^38.46 < p, and a reduced instance with all 16 inverse rounds was solved in 92 s (`audit/campaign.md` §3.2). The 0.3 record is kept in `vectors/hemera.json` under "0.3".

**12 full and 48 partial rounds.** Set by a rule against the published attacks (`audit/campaign.md` §3.3): each outer full layer keeps at least 3× the full rounds a published attack crosses or skips (2 → 6 per side), and the partial layer at least 1.5× the longest published subspace trail through the internal layer (≈ 30 → ≥ 45; 48). The Poseidon2 inequalities need R_P ≥ 14 at R_F = 12. Margins per attack line: [[permutation]].

**Poseidon2 reference matrices.** M4 is the reference block of Poseidon2 Appendix B; the internal diagonal is the reference Goldilocks t = 16 diagonal, reproduced from the Grain LFSR and the minimal-polynomial condition ([[matrices]]).

**self-bootstrapped constants.** Hemera₀ — the 0.4 structure with zero constants — absorbs the seed "cyber" and squeezes the 240 constants; checks and a counter rule make the procedure total ([[bootstrap]]).

**32-byte output.** 4 elements. A binary node fits in one rate block (32 + 32 = 64 bytes ≤ 8 × 8 bytes of rate when digests enter as full limbs), so tree hashing costs one permutation per node. Collision resistance of the 32-byte output is 128-bit classical, ~85-bit quantum; see the digest-length table for longer options.

## Specification pages

- [[field]] — Goldilocks prime field (canonical spec in [[nebu]])
- [[permutation]] — Poseidon2 round structure: S-box, linear layers, complete algorithm
- [[sponge]] — absorb/squeeze, padding, operational semantics
- [[capacity]] — structured capacity: flags, domain tags, counters, namespace bounds
- [[encoding]] — 7-byte canonical encoding, byte-to-field mapping
- [[tree]] — binary Merkle tree, `hash_node` construction
- [[constants]] — all 240 round constants (hex values)
- [[bootstrap]] — round constant self-generation via Hemera₀
- [[matrices]] — MDS and diagonal matrices for the linear layer
- [[api]] — public API surface: `hash`, `hash_node`, `absorb`, `squeeze`

## See also

- [[particles|particle]] — particle addressing with Hemera
- [[cyberlinks|cyberlink]] — edges referencing particles by Hemera hash
- [[cybergraph]] — the graph Hemera addresses
- [[nox]] — the VM where Hemera executes as a jet
- [[tri-kernel]] — probability engine consuming Hemera outputs
- [[cyber/proofs]] — [[zheng]] proof system built on Hemera
- [[BBG]] — authenticated state whose Lens commitments are hemera Merkle trees
- [[lens]] — polynomial commitment schemes (WHIR, TensorRs); hemera builds their Merkle trees and authentication paths
- [[cyber/whitepaper]] — section 4 Hemera chapter
