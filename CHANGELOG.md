# Changelog

## [Unreleased]

### Changed (breaking)

- `batch::verify_batch(chunks, proof, expected_root)` checks the reconstructed root against a caller-supplied root, like `tree::verify_proof`; `proof.root` is prover-controlled and no longer trusted. CLI: `hemera verify-batch <data> <proof> <root>`

### Fixed

- `stream::decode` rejects a header whose declared length exceeds the buffer before allocating or computing `HEADER_SIZE + data_len` (a tampered header aborted the process instead of returning `Truncated`)
- Invariant checks that were `debug_assert!` now hold in release builds: `StepSponge::absorb/step/squeeze`, the `stream` subtree walkers and `stream::left_subtree_chunks`; `cyber-hemera-wgsl` uses the shared `left_subtree_chunks` instead of a private copy
- CLI: a non-ASCII hash argument of the right byte length is rejected instead of panicking on a mid-character slice

### Tests

- `stream_async` header mismatch, truncation and zero-length paths; `async_io` read/write/flush loops and error paths

## [0.4.0] — 2026-10-16

Profile 0.4: the permutation changes, every output changes. Integration line `release/0.4`; `main` stays 0.3 until the owner promotes. The 0.3 vectors are kept under `"0.3"` in `vectors/hemera.json`. Decision and evidence: `audit/campaign.md` §0, §3.2; receipt `audit/profile-0.4-2026-10.md`.

### Changed (breaking)

- Permutation: x⁷ in every round (the x⁻¹ partial S-box is gone), R_F = 12 (6 + 6), R_P = 48 (`ROUNDS_F`, `ROUNDS_P`, `ROUNDS_TOTAL` = 12, 48, 60). Width 16, rate 8, capacity 8 and the 32-byte digest are unchanged
- External 4×4 block is the Poseidon2 reference matrix [[5,7,1,3],[4,6,1,1],[1,3,5,7],[1,1,4,6]] (was circ(2,3,1,1)); the internal diagonal is unchanged (it is the reference `MAT_DIAG16_M_1`)
- Round constants: 240 (192 external + 48 internal; `NUM_CONSTANTS`, new `constants::NUM_EXTERNAL`), self-bootstrapped through the 0.4 Hemera₀ with the constant checks and a counter rule (`bootstrap::bootstrap`, `constant_checks`, `squeeze_constants`, `BOOTSTRAP_COUNTER`); accepted at counter 0
- `COLLISION_BITS` = 128 (the 32-byte digest's birthday bound; was 256)
- Trace API: 60 callbacks per permutation; `RoundVisitor::partial_round` receives `SboxWitnesses = [x², x³]` (was the inverse); `FullRoundWitnesses = [SboxWitnesses; 16]`
- `Profile::new(rf_half, rp, external, internal, diag)`: the partial S-box argument and `PartialSbox` are removed
- `Goldilocks::inv` removed (hemera computes no inversion)
- CDC gear table regenerated (it is the hemera hash of each byte)
- GPU mirror (`cyber-hemera-wgsl`): same profile; `gl_inv` removed
- `vectors/hemera.json` regenerated with a `version` field; new sections for the permutation, keyed hash, derive-key, XOF, leaves, nodes, fixed-chunk and CDC roots, and the campaign's reduced-round instances

### Added

- `rs/tests/poseidon2_conditions.rs`: M4 MDS, M_E and M_I invertible, charpoly(M_I^i) irreducible for i = 1..32, on the layers read off the implementation
- `rs/examples/vectors.rs` (emits the vector file), `rs/examples/agree.rs` + `audit/campaign/agree.py` (crate against the Python model on random and edge inputs)
- `audit/campaign/poseidon2_params.py` (Grain LFSR, reference diagonal derivation, matrix checks) and the 0.4 model in `audit/campaign/hemera_ref.py`
- `permutation::permute_batch`, `tree::hash_node_batch`, `tree::hash_leaf_batch` — independent inputs run 16 at a time through a lane-interleaved kernel
- `permutation::Profile` / `HEMERA`, `permute_profile`, `permute_batch_profile` — the kernel is generic in round counts
- Constant-time kernel for secret input: `keyed_hash`, `derive_key` and their hashers run it internally; `hash_secret` / `Hasher::new_secret` (same digest as `hash`, constant time) and `permutation::permute_ct`. Contract in `specs/api.md` § constant time
- `reference` (doc-hidden) definitional oracle and `rs/tests/differential.rs` (10⁶ random inputs per family)

### Performance (M4 Max, shared machine, alternating binaries)

- One permutation 2.97 → 1.35 µs (2.2×): the 69-deep inversion chains are gone
- Batched 0.73 → 1.23 µs per permutation (0.6×): 48 partial rounds with a dense internal diagonal cost 1 728 multiplications, where 0.3 shared one inversion per partial round across 16 lanes

### Docs

- Specs state the 0.4 profile, its margins per attack line, the matrix derivation and conditions, the 240 constants, and the digest-length options (32 / 48 / 64 B); the 2^1046 / 2^918 claims, the x⁻¹ witness equations and the 16×16 MDS claim are removed; collision 128 bits and preimage 2^256 for the 32-byte digest

## [0.3.0] — 2026-05-12

### Added

- Async FSM verified streaming (`stream_async`) — O(log n) decoder for pre-order interleaved format
- `chunk_cv` and `parent_cv` aliases for chaining value accessors
- Sparse Merkle tree with 256-bit keys, depth-256, compressed proofs (bitmask encoding for sentinel siblings)
- Batch Merkle proofs — deduplicated multi-leaf proofs with depth-first sibling ordering
- GPU backend (`cyber-hemera-wgsl`) — wgpu compute shaders for batch_permute, hash_leaves, hash_nodes, root_hash, outboard; u64 emulated as vec2<u32>
- Zero-alloc hot paths in the CPU sponge and tree builder

### Changed

- Canonical parameters corrected to spec: R_P=16, x⁻¹ partial S-box, 32-byte (4-element) output, ~736 STARK constraints, 24 total rounds
- `specs/` replaces `reference/` as the directory for the full decision record
- Polynomial nouns integrated throughout specs and docs

### Fixed

- README link `[reference](reference/)` updated to `[specs](specs/)`

## [0.2.0]

Initial public release: Poseidon2 permutation over Goldilocks field (t=16, R_F=8, R_P=16), structured-capacity sponge, content tree (4 KB chunks, left-balanced Merkle), verified streaming, self-bootstrapped constants from seed `b"cyber"`.
