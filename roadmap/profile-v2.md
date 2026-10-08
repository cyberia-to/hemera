---
tags: cyber, cip, hemera, roadmap
crystal-type: process
crystal-domain: crypto
status: draft
breaks_hash: yes
date: 2026-10-08
---
# profile v2 — one permutation, post-quantum margin

replace the experimental inverse-16 profile with a single Poseidon2 instance whose parameters are chosen by a stated rule: x⁷ in every round, round counts set against the best published attacks with a factor for AI-assisted cryptanalysis, a digest sized for 128-bit quantum collision resistance, and reference matrices. one profile for everything — no permanent/ephemeral split, no modes. the hash changes; this is a genesis decision and the window closes when the genesis pipeline runs.

companion analysis: `cyber/audit/cryptography-2026-10-08.md` §5. research models referenced below live in PR #2 (`research/`).

## why the inverse profile goes

the inverse profile was adopted on one argument: degree p−2 per partial round gives 2^1046 algebraic degree, so sixteen partial rounds suffice ([[inversion-sbox]]). that argument is withdrawn (`specs/README.md`, `research/permutation-decision.md`: "the original justification … fails"). the reason is simple: an attacker never meets degree p−2. the inverse is modelled as x·y = 1, one quadratic equation — the same algebraic cost as the cheapest S-box — and polynomial-system solving is the one attack line that is advancing (Ethereum's 2026 estimates on Poseidon2 fell from 2¹⁶⁴ to 2¹²⁶ on exactly that line). against it the inverse profile has:

- fewer equations of the same kind — 16 partial rounds against the reference 22;
- extra structure — the 0→0 branch adds `x(z−1)=0, y(z−1)=0` to every partial round (`research/full-round-status.md`);
- an open subspace restriction — 15 of 16 partial rounds admit an affine line on which every active S-box is the identity (`research/partial_subspace.py`); eprint 2026/1692 uses exactly this ("algebraic cancellation of non-linear inverse S-boxes") to reach 28 of 31 rounds on Poseidon-KoalaBear;
- non-MDS linear layers (`specs/matrices.md`), the component eprint 2026/306 attacks;
- no published analysis, no bounty exposure: custom S-box and constants mean none of Poseidon2's bounds transfer;
- and no efficiency case: 6.67% fewer constraints than the reference at 2.85× the native multiplications (`research/permutation-decision.md`).

the strongest true statement about inverse-16 is "no evidence either way". for the identity of every particle, frozen at genesis without algorithm agility, that is below the bar.

## the rule

parameters are chosen by a rule so they can be re-derived when the attacks move.

- S-box: x⁷ in every round. the Poseidon2 reference S-box over Goldilocks; every published bound and every bounty instance is about it.
- full rounds: the best published attack strips 2 of 4 initial full rounds. keep at least 3× that intact on each side → R_F = 12 (6 + 6).
- partial rounds: the best published attack reaches ~14 of 23 partial rounds (CICO-1, eprint 2026/1692, KoalaBear t=24). rule: best known reach ≤ ½ of the budget, with a ×1.5–2 factor for AI-assisted progress → R_P in 44–56; point estimate R_P = 48.
- matrices: the Poseidon2 reference construction — external M_E from circulant 4×4 MDS blocks, internal M_I = I + diag(μ) with the published invertibility and eigenvalue conditions. both current matrices fail MDS.
- constants: keep the self-bootstrap (Hemera₀ sponge over the seed `cyber`), then run the Poseidon parameter checks on the result — invariant-subspace trails through the partial layer and matrix conditions. constants that fail are regenerated with a counter.
- width and sponge: t = 16, rate 8, capacity 8 (512 bits) unchanged.
- digest: 6 elements (48 bytes). see below.
- one profile. no keyed mode, no typed capacity, no second round count for ephemeral use. [[one-pure-hash]] stands.

## post-quantum digest

the digest, not the rounds, sets the generic quantum bounds. with capacity 512 bits the capacity-side bounds are 2²⁵⁶ classical and ~2¹⁷⁰ quantum (BHT); the digest is what binds:

| digest | classical collision | quantum collision (BHT 2^(n/3)) | NIST collision category |
|---|---|---|---|
| 4 elements, 32 B (current) | 2¹²⁸ | 2⁸⁵ | 2 |
| 6 elements, 48 B | 2¹⁹² | 2¹²⁸ | 4 |
| 8 elements, 64 B | 2²⁵⁶ | 2¹⁷⁰ (capacity-bound) | 5 |

[[compact-output]] chose 32 bytes knowing the 2⁸⁵ figure. a stated post-quantum requirement of 128-bit collision resistance means 48 bytes. 64 bytes buys 2¹⁷⁰ only because capacity is 512; it is the right choice only together with a wider state (below).

cost of 48 bytes: particle 48, link 96, +50% on every identity in storage and six limbs per id in circuits. a binary tree node becomes 96 bytes = 12 elements, above the rate of 8, so tree hashing takes two permutations per node instead of one — the gain [[compact-output]] bought is given back. the size ladder in `soft3/specs/terms.md` (atom 8 · pair 16 · particle 32 · link 64) becomes 8 · 16 · 48 · 96 — still length-discriminated, still tag-free, but a stack-wide change.

alternative that keeps one permutation per tree node: t = 24, rate 16, capacity 8, digest 8 elements (64 B). a node (128 B = 16 elements) fills one rate block; input rate doubles to 112 B per permutation; quantum collision 2¹⁷⁰. per permutation ~1.5× the cost of t=16; per byte of input ~0.75×. t=24 is a Poseidon2 reference width. the power-of-two convention is broken by the width only.

| | t=16 · 48 B | t=24 · 64 B |
|---|---|---|
| quantum collision | 2¹²⁸ | 2¹⁷⁰ |
| tree node | 2 permutations | 1 permutation |
| input rate | 56 B | 112 B |
| per-permutation circuit cost (RF12/RP48, S-box only) | 960 | 1344 |
| per-byte streaming cost | 1 | ~0.7 |
| identity size | 48 B | 64 B |
| size ladder | 8·16·48·96 | 8·16·64·128 |

recommendation: t=24 · 64 B if tree hashing throughput matters at genesis scale (BBG, Brakedown columns); t=16 · 48 B if identity size matters more. both satisfy the requirement; neither does with 32 bytes.

## cost of one profile

all figures in Hemera's own accounting (`[[inversion-sbox]]`: S-box 4 constraints for x⁷, 2 for x⁻¹; MDS ~8 per round; native: full round 112 muls, partial x⁷ round 20 muls, partial x⁻¹ round 81 muls).

| profile | S-boxes | S-box constraints | with MDS | native muls / perm | native, relative |
|---|---|---|---|---|---|
| inverse RF8/RP16 (current) | 144 | 544 | ~736 | ~2224 | 1.00 |
| x⁷ RF8/RP22 (reference) | 150 | 600 | ~840 | ~1336 | 0.60 |
| x⁷ RF8/RP44 | 172 | 688 | ~1104 | ~1776 | 0.80 |
| x⁷ RF12/RP48 (proposed) | 240 | 960 | ~1440 | ~2304 | 1.04 |
| Keccak-f, for scale | — | ~25 000 | — | — | — |

native throughput is unchanged (~53 MB/s today, ~51 MB/s proposed; t=24 variant higher per byte). in-circuit cost per permutation rises 76% on S-box constraints, 96% with explicit MDS, 0% of the MDS part once [[constraint-free-mds]] lands. at the stack level the overhead is the hashing share of a proof times that factor: today's Brakedown-Merkle recursion is hash-heavy, so up to roughly +50–70% on those proofs; the endgame in this roadmap (~3 Hemera calls per execution, Merkle-free lens, algebraic NMT) makes it a few percent. the second profile idea saves at most the difference on the ephemeral share and costs a second analysis target, a second bounty, and the mode variance [[one-pure-hash]] removed. one profile.

## what changes

- `rs/src/params.rs`: ROUNDS_F 8→12, ROUNDS_P 16→48, partial S-box x⁷, matrices regenerated, OUTPUT 4→6 (or width 24 / output 8); WGSL mirror.
- `specs/`: README parameter block and security claims (generic collision 2¹⁹² classical / 2¹²⁸ quantum; algebraic margin: measured by bounty, not claimed), matrices.md, constants.md (regenerated constants + the parameter-check record), capacity.md, encoding.md, bibliography.
- `vectors/`: all known-answer tests regenerate.
- `research/` (PR #2): `full_round_system.py` and `partial_subspace.py` re-run on the new profile; the subspace certificate becomes the first bounty instance.
- cross-repo: `file::Particle` width, `soft3/specs/terms.md` size ladder, bbg node layout, lens column digests, foculus win-test preimage, the genesis pipeline (3,143,650 particles rehash from bytes; 97.63% present; the 2.37% black holes stay addressed by CID label as decided in `cyber/launch.md`).
- a public bounty: reduced-round CICO and preimage instances of the new profile, in the EF format, before genesis.

## open

- R_P = 44, 48 or 56 — the AI factor is a judgement.
- t=16 · 48 B against t=24 · 64 B — identity size against tree throughput.
- whether to adopt the Poseidon2 reference constants outright (inherits the published instance exactly, loses the `cyber` seed) or keep the self-bootstrap with the parameter checks (keeps the seed, inherits the family's analysis, not the instance's).
- timing: this is a genesis decision. after 2026-11-05 it is a fork.

see [[inversion-sbox]] · [[compact-output]] · [[one-pure-hash]] · [[constraint-free-mds]] · `cyber/audit/cryptography-2026-10-08.md`
