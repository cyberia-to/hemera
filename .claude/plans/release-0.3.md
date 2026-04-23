# Release 0.3.0 Plan

## Summary

Prepare cyber-hemera workspace for 0.3.0 release on crates.io and GitHub.

## Changes since 0.2.0

- async FSM verified streaming (`stream_async`)
- `chunk_cv`/`parent_cv` aliases for compatibility
- aligned code with spec — R_P=16, x^(-1) partial S-box, 32-byte output
- canonical params fix — Rp=16, 32-byte output, ~736 constraints, 24 rounds
- reference/ → specs/ restructure
- polynomial nouns integration

## Tasks

### 1. Version bump (all crates)

0.2.0 → 0.3.0 in:
- `rs/Cargo.toml` (cyber-hemera)
- `wgsl/Cargo.toml` (cyber-hemera-wgsl)
- `cli/Cargo.toml` (hemera-cli)
- `bench/Cargo.toml` (hemera-bench)

### 2. Fix path dependencies for crates.io

Path-only deps fail `cargo publish`. Add version specifiers:

- `wgsl/Cargo.toml`: `cyber-hemera = { path = "../rs", version = "0.3.0" }`
- `cli/Cargo.toml`: both `cyber-hemera` and `cyber-hemera-wgsl` need versions

### 3. Add missing crates.io metadata

`wgsl/Cargo.toml` — add: homepage, documentation, keywords, categories, readme
`cli/Cargo.toml` — add: homepage, documentation, keywords, categories, readme

### 4. Fix README

- `See [reference](reference/)` → `See [specs](specs/)` (line 97)

### 5. Create CHANGELOG.md

Conventional changelog covering 0.2.0 → 0.3.0 changes.

### 6. Verify

- `cargo publish --dry-run -p cyber-hemera`
- `cargo publish --dry-run -p cyber-hemera-wgsl`
- `cargo test --workspace`
- `cargo clippy --all-features`
- `cargo doc --all-features --no-deps`

### 7. Release

- Commit all changes
- Tag `v0.3.0`
- `cargo publish -p cyber-hemera` (core first)
- `cargo publish -p cyber-hemera-wgsl` (depends on core)
- Create GitHub release with changelog

### Publish order

1. `cyber-hemera` (no deps on workspace)
2. `cyber-hemera-wgsl` (depends on cyber-hemera)
3. `hemera-cli` — optional, skip if not needed on crates.io (depends on both)
4. `hemera-bench` — `publish = false`, skip

## Decision points

- Should `hemera-cli` be published to crates.io? (`cargo install hemera-cli`)
- Should we add a GitHub Actions release workflow for future releases?
