# Changelog

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
