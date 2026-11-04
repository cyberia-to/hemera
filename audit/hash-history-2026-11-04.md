---
tags: hemera, audit, particle
crystal-type: entity
crystal-domain: crypto
date: 2026-11-04
---
# what is hashed and how — the history, 2026-03-14 to 2026-11-04

for the owner's statement of 2026-11-04 on hemera: a hash that simply works, without options. this page gathers the record and makes no recommendation. read against `origin/main` fd55f99 and `origin/release/0.4` 8559b52.

authorship: every hemera commit carries the owner's git identity. agent work shows only through `Co-Authored-By` trailers, written 2026-03-14..05-18 (Opus 4.6, Opus 4.7, Sonnet 4.6). from 2026-05-26 no trailers are written, so git alone does not separate the owner's hand from an agent's. "no trailer" below means only that.

## 1. timeline

| date | decision about what is hashed | source | author | stands now |
|---|---|---|---|---|
| 03-14 | content tree: fixed 4096-byte chunks, left-balanced; `tree_hash` renamed `root_hash`, "make tree hash canonical" | 0306b6c, ca786b2, 2541a83 | agent (Opus 4.6) | survives as `fixed_chunk_root` |
| 03-14 | spec imported from the cyber graph: "Chunk Size: 4 KB … No content-defined chunking — every boundary is deterministic" | 5d0df2a, `specs/tree.md` | agent import of graph text | still the text on main and release/0.4 |
| 03-15 | verified streaming over the 4 KB tree | 2d824a7 | agent (Opus 4.6) | implemented, on `fixed_chunk_root` |
| 03-16 | BLAKE3 modes in `state[11]`: hash / keyed / derive_key | 0278115 per `roadmap/one-pure-hash.md` §3 | agent (Opus 4.6) | in code; `specs/capacity.md` "planned for removal" (bb27ff2, 06-09) |
| 03-18 | 32-byte output | 13901e2 | no trailer | kept in 0.4 |
| 03-25 | `noun_id = hemera(Lens.commit(noun) ‖ tag)`; tree hashing "remains available for legacy compatibility" | 37a3f84, `specs/tree.md` § Polynomial Noun Identity; bbg fbc3cee | agent (Opus 4.6) | text only, no code |
| 04-23 | semantic hashing, status draft: section tree + element-aligned CDC; "frontmatter is not a section … identity is insensitive to frontmatter serialization" | cc78820 | the owner (no trailer) | rewritten |
| 05-12..14 | eight rewrites, draft → accepted: "replaces fixed-4KB chunking in `particle_id` for all content"; frontmatter is section[0]; `[[files]]\n` split; author-declared `element`; "the eternal hash" | 317e4d7, 9a19dc8, 128c12e, 5a1f01c, 5397399, 57155aa, 7e2a46a, dfa4dde | agent (Sonnet 4.6 on each) | page says accepted; `roadmap/README.md` says "in reference? no" |
| 05-14 | `root_hash` = `section_cdc_tree(data, 1, true)`; 4 KB tree kept as `fixed_chunk_root` for streaming and proofs | ed56504, `rs/src/cdc.rs`, `rs/src/tree.rs:205` | agent (Sonnet 4.6) | in code; no CHANGELOG entry; `specs/tree.md` unchanged |
| 06-08 | one pure hash, draft: "hemera is `hash(data) → particle` and nothing else"; drop modes; type goes into structure `pair(domain, value)`, not capacity | cffa979 | no trailer | draft; modes still in code |
| 09-10..12 | structural commitments: framed blobs, sequences, records, .cyb adapter over CDC; "not a migration of `root_hash`"; "not frozen for mainnet" | 7407dec, PR #2 (open, DECIDE) | agent research PR | not on main |
| 09-16 | particle = hemera hash of a file's data; `file::Particle::hash = hemera::hash`; same day cyber/particle.md names bbg's `hemera(Lens.commit(data) ‖ PARTICLE)` | hemera 4a6d1d4, b365c9a; file 68b2be0; cyber 6e41857a | no trailers | both texts stand |
| 09-18 | "one particle. three definitions are in play"; freezes before the genesis pipeline | cyber `launch.md` critical dependencies 1 | launch page | open |
| 09-19..25 | survey: five definitions; every code path hashes the whole slice in one call; no code implements the spec's 4 KB tree | cybergraph#4, `cybergraph/audit/particle-alias-survey.md`; launch row 19 | agent (launch worker) | decision open |
| 09-24 | "every crate computes hemera(data) today (file, bbg, cybergraph); only cyber/particle.md states the lens construction" | `launch.md` decisions log | launch page | open |
| 10-09 | cybergraph particle spec 64 → 32 bytes, 4 KB tree text kept | cybergraph 47c7b4a | no trailer | 32 bytes |
| 10-16 | profile 0.4: every output changes, the 32-byte identity format does not; gear table regenerated (it is `hemera::hash` of each byte); "one profile for everything — no permanent/ephemeral split, no modes" | PR #15, PR #21, CHANGELOG 0.4.0, `launch.md` decision 10-16 | decision log, agent PRs | release/0.4; PR #25 awaits the owner |
| 10-16 | genesis on 0.4: chain id = `hemera::hash` of the genesis file bytes, flat; every genesis particle = `hemera(cid bytes)` | soft3 `crate/src/genesis/mod.rs:76,188`; cybernode `specs/chain.md` §3.1, §4.1, `specs/genesis.md`; launch row 14 (pussy `bae85f40…`) | — | frozen by pussy |
| 11-04 | "a hash that simply works, without options" | the owner, on hemera | the owner | this page |

## 2. four particle constructions

| | construction | written in | runs today |
|---|---|---|---|
| A | flat `hemera::hash(bytes)`: one sponge, mode 0, no tree, no flags | `file/src/lib.rs:34`; cyber `file.md`; cybernode `chain.md` §3.1 | file, spark, cyb; tru `particle_of`; soft3 genesis and chain ids; bbg, cybergraph, foculus take the 32 bytes as given. pussy's genesis freezes A |
| B | 4 KB tree: leaf `Hemera(chunk)`, node `Hemera(l ‖ r)`, ≤ 4 KB → `Hemera(content)`; input prefixes 0x01–0x04 | `cybergraph/specs/particle.md` | no code (survey item 5). hemera's own 4 KB tree (`fixed_chunk_root`, counter and flags in capacity) differs from B; it runs in streaming, proofs, CLI, GPU |
| C | `hemera(Lens.commit(data) ‖ PARTICLE)` | cyber `particle.md`; bbg `docs/explanation/architecture-overview.md:69`; hemera `specs/tree.md` | no code (`launch.md` decisions log) |
| D | CDC `tree::root_hash` = `section_cdc_tree(bytes, 1, true)` | `rs/src/tree.rs:205`, `rs/src/cdc.rs`; `roadmap/semantic-hashing.md` | hemera library; soft3 conformance `hemera::tree::root_hash@v1` beside `hash@v1` and `fixed_chunk_root@v1` (`conformance/rs/src/surfaces/hemera.rs:54-62`); no particle uses it |

they disagree: `hemera::hash("hello")` = `85cbee6d…`, the tree vector for "hello" = `a384f241…` (`vectors/hemera.json`; a single-chunk root is a flagged leaf). A = B only up to 4 KB; D = hemera's 4 KB tree only below ~2 KB (`root_hash` doc); C equals none. inside hemera `root_hash` names two things: `tree::root_hash` is D, while `root_hash_with_progress` (CLI) and the GPU `root_hash` compute the 4 KB tree (`rs/src/tree.rs:228`, `cli/src/main.rs:109-115`, `wgsl/tests/gpu.rs`). unchecked: `wgsl/src/lib.rs:733` may call the CDC `root_hash` where a fixed-chunk subtree is meant.

## 3. where a hashing choice made serialization a requirement

| hashing choice | requirement it created | source |
|---|---|---|
| .cyb section tree (Sonnet rewrite) | frontmatter is section[0], split on `[[files]]\n` by byte pattern; "frontmatter values must be single-line"; LF canonicalization "ensures identical section bytes everywhere"; `~~~` excluded; non-multiple sizes reject | `roadmap/semantic-hashing.md` 135–197, 490–494, 537; 9a19dc8 |
| the same, owner's draft | "identity is insensitive to frontmatter serialization" | cc78820 |
| element-aligned CDC | `element = N` (1..64) per section, "absent → 1, no inference"; a wrong value "silently misaligns CDC boundaries" | 5a1f01c, 57155aa |
| CDC in `root_hash` | identity depends on the gear table (`hemera::hash` of each byte, regenerated in 0.4), W and boundary rules | `cdc.rs` header; CHANGELOG 0.4.0 |
| structural commitments | `hemera-struct-v1\0` + tag, u64 LE, kinds `cyb.section.v1` / `cyb.entry.v1` / `cyb.container.v1`; "framing is injective"; "settle TOML multiline values, duplicate names, trailing data and delimiter reconstruction"; "freeze primitive, encoding and cross-language vectors together"; an opening does not prove the producer "chose canonical CDC boundaries" | PR #2, `specs/structural-commitments.md` |
| type out of capacity | nominal type as `pair(domain, value)`: the type enters the hashed bytes | `roadmap/one-pure-hash.md` §1 |
| domain separation at input | prefixes 0x01 edge … 0x04 node; "domain separation lives in the hash input (different serialization, different capacity flags)" | `cybergraph/specs/particle.md`; `docs/explanation/particle-ids.md:40` |
| cyberlink identity | "the sponge hash of its canonical encoding" | `specs/sponge.md:38` |
| chain identities | `hemera("soft3/chain/leaf/v1\0" ‖ tx)`, txid, statement; 168-byte header; genesis is a tade stream "in the canonical order below, nothing else", its particle is the chain id | cybernode `specs/chain.md` §1, §3.1, §4.1 |
| coordinate particles | "the hemera hash of the canonical bytes `"locus:" ‖ E ‖ r ‖ θ`" | `cybergraph/specs/address.md:20` |
| lens construction C | identity depends on `Lens.commit`; "the choice also fixes the layer of lens" | cyber `particle.md`; `launch.md` decisions log |
| review doctrine | "serialization is canonical — single valid encoding per value" | hemera `AGENTS.md:219` |

the texts where the hash takes bytes as they are: "identity is never on the wire as a primitive; a particle is computed from the bytes at either end" (`tade/README.md:13`); "on the wire a file is a tade frame … particle = hemera(data)" (cyber `file.md`); "no version prefix, no mode byte" (`specs/encoding.md:54`); "same bytes → same hash … (distinct from CIDv0/UnixFS)" (cybics `crypto/features.md:95`).

## 4. open questions, as stated

- which definition wins, hemera(data) or hemera over the lens commitment; blocks row 20 — `launch.md` decisions log, row 19, critical dependencies 1 (its "64-byte" cybergraph note is stale since 47c7b4a)
- bostrom re-addressing "needs the decision in row 19" — `launch.md` row 20, file#2
- width, domain separation, chunking, ownership — `cybergraph/audit/particle-alias-survey.md`
- semantic hashing "accepted" but spec targets unwritten (`specs/tree.md`, `particle_id(&CybFile)` in `specs/api.md`, `## identity` in cyb `reference/format.md`) — `roadmap/semantic-hashing.md`, `roadmap/README.md`
- "Decide: merge after eidos#1"; six migration gates — PR #2
- "does anything beyond radio need a fast symmetric MAC / KDF … confirm before deleting the modes" — `roadmap/one-pure-hash.md`
- 48-byte digest "is a separate decision (it changes the particle size and the tree node layout)" — `specs/README.md` § digest length, PR #15
- 0.4 on main needs "the owner's approval" — PR #25
- `audit/campaign.md` §1.2 lists particles under both plain `hash` and the content tree

no recommendation. the owner will think.
