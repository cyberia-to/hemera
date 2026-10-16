---
tags: hemera, audit, profile, permutation, performance
crystal-type: audit
crystal-domain: crypto
date: 2026-10-16
---
# hemera 0.4 profile — receipt, 2026-10-16

Decision 2026-10-16 (delegated by the owner, on the measured finding of
`audit/campaign.md` §3.2, merged as hemera#20): the frozen material of the
phase-1 root chain is the hemera#15 permutation. Implemented as crate
`cyber-hemera` 0.4.0 on the integration line `release/0.4` (branched from
`main` at `1c385e5`); `main` stays 0.3 until the owner promotes.

## 1. the profile

| | 0.3 (record) | 0.4 |
|---|---|---|
| field | Goldilocks p = 2^64 − 2^32 + 1 | same |
| width / rate / capacity | 16 / 8 / 8 | same (kept) |
| full rounds | 4 + 4, x⁷ | 6 + 6, x⁷ |
| partial rounds | 16, x⁻¹ (0 ↦ 0) | 48, x⁷ |
| initial layer | M_E | same |
| M4 (external block) | circ(2, 3, 1, 1) (Plonky3 `MDSMat4`) | [[5,7,1,3],[4,6,1,1],[1,3,5,7],[1,1,4,6]] (Poseidon2 Appendix B, Horizen Labs) |
| internal diagonal | `MAT_DIAG16_M_1` | same values — they are the reference diagonal |
| constants | 144, bootstrap from "cyber" | 240, bootstrap from "cyber" through the 0.4 Hemera₀, checks C1–C3, counter 0 |
| constants sha-256 (240 × u64 LE) | — | `a70d1f9881707585db210504746ba61802b1a3b6ec83e6ea85a42dfebb9e0cd5` |
| first constant | `0x7E6EF67C13BC8100` | `0x8BF262E620238C97` |
| sponge, padding, domain tags, tree flags | — | unchanged |
| digest | 4 limbs, 32 B | 4 limbs, 32 B (kept; 48 / 64 B rows in `specs/README.md`) |
| multiplications per permutation | ~2 224 | 1 728 |
| S-boxes / constraints (hemera accounting) | 144 / ~736 | 240 / ~1 440 |
| hash("") | `a67a71b2…806d563c` | `9e89f2487be432e48c27e24e6945adffe6918c8b5d6a62061e70e0bebd657326` |
| `vectors/hemera.json` sha-256 | — | `5b88d88aab157e455b2b48760f1e369b0ba3725a34ddad4236bc567fd5fdda5f` (version 0.4.0, 0.3 record under `"0.3"`) |

## 2. correctness

Release build, aarch64 assembly path unless stated.

| check | result |
|---|---|
| unit tests (`cyber-hemera`) | 242 passed |
| `rs/tests/differential.rs` against the definitional oracle (explicit 16×16 matrix products, plain x⁷) | 10⁶ permutations (single and batched, every lane group), 10⁶ hashes 0..200 B, 10⁶ `hash_node` pairs (single and batched), 125 000 leaves, 250 000 batched leaves, 4 other round shapes × 100 000: 0 mismatches |
| `rs/tests/constant_time.rs` | `permute_ct` = `permute` bit for bit on 10⁶ states; secret entry points against the oracle sponge on 10⁵ inputs each |
| the same three suites with `--cfg hemera_portable` (no assembly) | pass |
| `rs/tests/vectors.rs` | every entry of `vectors/hemera.json`, fast and constant-time kernels and the oracle; the 0.3 section is present and differs |
| `rs/tests/poseidon2_conditions.rs` | layers read off the implementation equal M_E = circ(2·M4, M4, M4, M4), M_I = 1 + diag(d); M4 MDS (69 minors); M_E, M_I invertible; charpoly(M_I^i) irreducible for i = 1..32; negative control |
| `cyber-hemera-wgsl` GPU tests on Metal (M4 Max) | 45 passed against the CPU |
| clippy | no warnings in the files this change touched |

independent model, `audit/campaign/` (pure Python, derives every constant):

| check | result |
|---|---|
| `poseidon2_params.py`: Grain LFSR vs Horizen Labs width-8 Goldilocks constants | equal |
| reference diagonal search (Grain + minimal-polynomial condition), t = 8 | candidate 3 = `MAT_DIAG8_M_1` |
| same, t = 16 | candidates 0–45 rejected, candidate 46 = `MAT_DIAG16_M_1` |
| matrix checks on the 0.4 layers | M4 MDS, M_E and M_I invertible, charpoly(M_I^i) irreducible i = 1..32 (cross-checked with python-flint 0.9 factorisation) |
| `hemera_ref.py`: generic model at t = 8 with HL constants vs the 3 HL/Plonky3 test vectors | equal |
| 0.4 bootstrap vs `rs/src/constants.rs` and `specs/constants.md` | 240 / 240 equal; checks C1–C3 pass at counter 0 |
| CDC gear table vs `rs/src/cdc.rs` | 256 / 256 equal |
| every 0.4 vector, and the 0.3 section against the 0.3 model | all equal |
| `agree.py`: crate vs model, random and edge inputs | 20 000 permutations (zeros, ones, p − 1, p, 2^64 − 1, unit vectors, non-canonical limbs), 5 000 hashes 0..300 B, 5 000 nodes (incl. all-0xff digests): all equal |

## 3. performance

Apple M4 Max (12 P + 4 E), macOS 26.4.1, rustc 1.95.0, default release
profile. The machine was shared throughout (1-minute load average 19–29
during the run below, up to 75 in others); macOS offers no core pinning.
Method: `rs/examples/bench.rs` (median of 31 runs per row), the 0.3
binary (`origin/main` 1c385e5) and the 0.4 binary alternated, 7 rounds
each; the table gives the median and the minimum of the 7 per-round
medians, ns per operation.

| operation | 0.3 median | 0.3 min | 0.4 median | 0.4 min | 0.3 / 0.4 |
|---|---|---|---|---|---|
| Goldilocks mul, dependent chain | 2.2 | 2.0 | 2.0 | 2.0 | — |
| permute, single (latency) | 2 970 | 2 725 | 1 351 | 1 345 | 2.20× |
| permute_ct, single | 3 678 | 3 239 | 1 625 | 1 581 | 2.26× |
| hash 64 B (2 permutations) | 6 433 | 5 551 | 3 793 | 2 759 | 1.70× |
| hash_secret 64 B (CT) | 7 416 | 6 662 | 4 200 | 3 283 | 1.77× |
| keyed_hash 64 B (CT) | 6 821 | 6 588 | 3 453 | 3 317 | 1.98× |
| hash_node, single (latency) | 2 778 | 2 751 | 1 370 | 1 339 | 2.03× |
| 1024 × permute (loop) | 2 730 | 2 688 | 1 360 | 1 334 | 2.01× |
| 1024 × permute_batch | 732 | 714 | 1 227 | 1 207 | 0.60× |
| 1024 × hash_node (loop) | 2 731 | 2 698 | 1 355 | 1 338 | 2.02× |
| 1024 × hash_node_batch | 730 | 712 | 1 227 | 1 207 | 0.60× |
| 1024 × hash_leaf 144 B (loop) | 11 115 | 10 913 | 5 598 | 5 545 | 1.99× |
| 1024 × hash_leaf_batch 144 B | 3 036 | 2 947 | 5 304 | 4 932 | 0.57× |

the table was measured on commit `fe1d64b`; the CI clean-up that followed
(rustfmt, clippy) was checked against it with 4 more alternating rounds
(load 55–90): permute single minimum 1 358 ns (merged) against 1 374 ns
(measured build), `permute_batch` 1 214 against 1 214 ns — no change.

permutations per second on one core: 0.3 ≈ 0.34 M single / 1.37 M
batched; 0.4 ≈ 0.74 M single / 0.81 M batched.

where the time goes (`rs/examples/split.rs`, 3 alternating rounds, median,
ns per permutation over 1 024 states):

| | 0.3 single | 0.3 batched | 0.4 single | 0.4 batched |
|---|---|---|---|---|
| whole permutation | 2 731–2 744 | 708–721 | 1 357–1 360 | 1 242–1 271 |
| full rounds only | 412–416 (8 rounds) | 387–393 | 614–636 (12 rounds) | 597–604 |
| partial rounds only | 2 304–2 335 (16 × x⁻¹) | 349 | 770–784 (48 × x⁷) | 690–704 |

reading:

- single permutation: 2.2× faster. 0.3's latency was the sixteen 69-deep
  inversion chains (2.3 µs of 2.7); a 0.4 partial round's serial path is
  x⁷ (three dependent multiplications) plus one multiply-add.
- batched: 1.7× slower per permutation. 0.3 shared one inversion per
  partial round across 16 lanes (Montgomery's trick), so its batched
  partial layer cost 349 ns; 0.4's 48 partial rounds each multiply the 15
  other lanes by the dense reference diagonal (768 multiplications per
  permutation), and full rounds grow from 8 to 12. The kernel is then
  throughput-bound: lane counts 4, 8 and 16 measured equal (1 186–1 195 ns
  minimum over 10 alternating runs; 2 lanes 1 219 ns), and a
  tree-shaped partial-round sum changed nothing measurable.
- the 0.4 kernel equals the 0.3 crate's own generic x⁷ path for the same
  shape (the 0.3 `split` example's "v2-shaped" rows: 1 323–1 333 single,
  1 208–1 211 batched), i.e. removing the inversion machinery left the x⁷
  path at its previous speed.
- the campaign's estimate "~2 304 against ~2 224 multiplications" compared
  0.4 with unbatched 0.3; measured, 0.4 has 1 728 and is faster single and
  slower batched. Consumers that hash many independent nodes per call
  (lens Merkle columns, BBG levels) see the batched row.

## 4. what changed in the code

- `rs/src/params.rs`, `constants.rs`, `profile.rs`, `field.rs` (HL M4,
  `inv` removed), `kernel.rs` (x⁷ partial rounds; inversion chain,
  Montgomery batch inversion and fill interleaving removed),
  `permutation.rs` (60-round stepping and tracing), `trace.rs` (partial
  witnesses `[x², x³]`), `bootstrap.rs` (checks + counter), `reference.rs`
  (explicit matrix oracle), `cdc.rs` (gear table), `arith.rs`.
- `wgsl/src/shaders/{params,permutation,field}.wgsl`.
- tests: `differential.rs`, `vectors.rs` (reads the JSON),
  `poseidon2_conditions.rs` (new); examples `vectors.rs`, `agree.rs` (new),
  `split.rs`, `bench.rs`.
- specs, docs, README, roadmap, CHANGELOG; `audit/campaign.md` retargeted.

## 5. dependents that must change (the sweep)

hemera's API changes: `ROUNDS_F` 12, `ROUNDS_P` 48, `ROUNDS_TOTAL` 60,
`NUM_CONSTANTS` 240 with the internal constants at index 192 (0.3: 128;
terminal rows start at 96, 0.3: 64), `COLLISION_BITS` 128,
`Goldilocks::inv` and `PartialSbox` removed, `Profile::new` without the
S-box argument, `RoundVisitor::partial_round` takes `[x², x³]`. Every
`version = "0.3"` requirement on `cyber-hemera` must move to 0.4 even with
`path =`. Survey of `~/cyber` on 2026-10-16 (several items live only on
branches):

- **lens GPU** (`gpu/` on `feat/zk-small`, `release/0.2`,
  `feat/mine-kernel`): `src/msl.rs` — `ginv`, the circ(2,3,1,1) `mat4` /
  `fexternal` schedules, `permute_ref` and `permute_k` with `RC[128+r]`,
  the 16-round Montgomery batch inversion and the `D0P1` carry,
  `constant ulong RC[144]`; `src/wgsl.rs` / `wgsl2.rs` — `K[160]`, `ginv`,
  `mds_ext`, offsets `128u` / `144u`, `permute2` shared inversion;
  `src/consts.rs` `[u64; 144]`; `src/webgpu.rs` constant upload;
  `tests/differential.rs` zero-inverse and shared-inversion tests; specs
  `polynomial-ring.md`, `commitment.md`, `binary-tower.md`,
  `isogeny-curves.md` (x⁻¹, ~736).
- **zheng**: `rs/src/execution/hash.rs` and `execution/tagged/hash.rs`
  (inverse witness gadget, 24 rounds, `128 + round − 4` offsets) — the node
  jet `rs/src/execution/node.rs` (`COST = 39_818`, INV/INVLOOP formula);
  `ccs/transcript.rs` (`RoundVisitor`, 24 rows), `ccs/universal.rs`
  (u_0..u_24 selectors, `RC[128+…]`, inverse witness), `ccs/particle.rs`,
  `ccs/root.rs` (14 × 24 rows); on `feat/block-aggregate` / `feat/zk-small`:
  `machine/hemera.rs`, `machine/layout.rs` (32-row PERM block, `PINV`
  column — 60 rounds do not fit), the 4-row hemera gadget
  `recursion/circuit/{layout,air,builder,trace}.rs` (`PERM_ROWS = 4`,
  inverse witness, rows after rounds 2–3 / 20–22); `specs/soundness.md`
  hash row and constraint counts.
- **nox**: `rs/patterns/hash.rs` (`ROUNDS_TOTAL = 24`, 25-row trace), hash
  pattern cost 25 → 61 in `reduce.rs`, `bound.rs`,
  `jets/merkle_verify.rs` (a budget decision), specs on the hash pattern.
- **soft3**: `release/phase1.toml` hemera pin (`23f3bbc`); miner Job
  sponge view `crate/src/chain/mine/job.rs` (sponge unchanged, so its
  layout stays; GPU and CPU miners follow lens and hemera); pinned digests
  in `crate/src/chain/{sigs,header,pow,tree,genesis,conformance}.rs`,
  `genesis/tests.rs`, `wallet/tests.rs` (the mined header in `pow.rs` must
  be re-mined); `specs/chain.md` hashrate figures; conformance snapshots
  (`mechanism.snap`, `encoding.snap` `hemera::params@v1`, `manifest.snap`,
  `chain.snap`, `chain-genesis.toml`, `provenance.toml`, zheng proof
  fixtures) and `conformance/rs/src/surfaces/hemera.rs`.
- **genesis particles**: soft3 `crate/src/genesis/derive.rs`
  (`particle_of_cid`, owners, passports) and `genesis/file.rs` — every
  bostrom particle (3 143 650) and owner id re-derives; `cyber/launch.md`
  rows 14, 20.
- **honeycrisp** `acpu/src/field/{permute,merkle}.rs`: a second CPU copy of
  the 0.3 permutation (`&[u64;144]`, `gl_inv`, circ(2,3,1,1)); no caller
  found.
- **bbg** `rs/src/root.rs` (follows hemera; pinned roots live in soft3).
- **pinned vectors elsewhere**: radio / kadek worktree branches
  (`iroh-blobs` empty-root and root-policy tests, `cyber-bao` tests,
  `kadek-ifac` provenance and bao-range fixtures).
- **documents with 0.3 numbers**: `cyber/whitepaper.md` §4.2 / §4.4 and
  later (R_F 8, R_P 16, 144 constants, 736), `cyber/litepaper.md`, zheng
  specs and docs, nox specs, lens specs, soft3 docs and site,
  `foculus/specs/provable-consensus.md`, `mudra/roadmap/goldilocks-fhe.md`,
  `trident/roadmap/polynomial-target.md`, `radio/docs/blake3-to-poseidon2-migration.md`.

## 6. reproduce

```
cargo test --release --workspace                         # all of §2 (Rust)
RUSTFLAGS="--cfg hemera_portable" cargo test --release -p cyber-hemera --test differential --test constant_time --test vectors
cargo run -p cyber-hemera --example vectors > vectors/hemera.json
python3 audit/campaign/poseidon2_params.py               # Grain, diagonal derivation, matrix checks
python3 audit/campaign/hemera_ref.py                     # constants, vectors (0.4 and 0.3), gear table
python3 audit/campaign/agree.py 20000 5000 5000          # crate vs model, random and edge inputs
cargo run --release -p cyber-hemera --example bench      # §3
cargo run --release -p cyber-hemera --example split
```
