# hemera

> **WARNING: This is novel, unaudited cryptography. The parameter set, sponge
> construction, and self-bootstrapping round constant generation have not been
> reviewed by third-party cryptographers. Do not use in production systems
> where cryptographic guarantees are required. Use at your own risk.**
>
> **The Hemera hash is not finalized and may change. Hash outputs, round
> constants, and the sponge construction are subject to breaking changes
> until a stable release.**

Poseidon2 hash over the Goldilocks field.

```
Field:           p = 2^64 - 2^32 + 1 (Goldilocks)
S-box:           d = 7  (x^7)
State width:     t = 16
Full rounds:     R_F = 8  (4 + 4)
Partial rounds:  R_P = 64
Rate:            r = 8  elements (56 bytes)
Capacity:        c = 8  elements (64 bytes)
Output:          8  elements (64 bytes)
Collision:       256 bits classical, 170 bits quantum
```

Every parameter is a power of two.

## Workspace

| Crate | Description |
|-------|-------------|
| `rs` | Core Rust implementation (`cyber-hemera` on crates.io) |
| `wgsl` | GPU backend — WGSL shader + wgpu dispatch |
| `cli` | CLI binary (`hemera`) |
| `bench` | Benchmarks (criterion) |

Three implementations cross-verify against shared test vectors in `vectors/`.

## Usage

```rust
use cyber_hemera::{hash, keyed_hash, derive_key, Hasher};

// One-shot
let digest = hash(b"hello world");

// Streaming
let mut hasher = Hasher::new();
hasher.update(b"hello ");
hasher.update(b"world");
assert_eq!(hasher.finalize(), digest);

// Keyed
let mac = keyed_hash(&[0u8; 64], b"message");

// Key derivation
let key = derive_key("my-app v1", b"key material");

// Content tree hash
let root = cyber_hemera::tree::root_hash(b"file contents");
```

## CLI

```
hemera file1.txt file2.txt    # hash files
echo hello | hemera           # hash stdin
hemera --check sums.txt       # verify checksums
```

Install: `cargo install --path cli`

## Self-bootstrapping round constants

Hemera generates its own round constants from a genesis seed `[0x63, 0x79, 0x62, 0x65, 0x72]` ("cyber") through a zero-constant Poseidon2 sponge. No external PRNG, no nothing-up-my-sleeve numbers from other sources.

## Rationale

Full specification and design rationale for the Hemera parameter set: [hemera-spec](http://cyber.page/hemera-spec). For broader context on why Hemera exists and how it fits into the Cyber network: [hemera](http://cyber.page/hemera).

## License

cyber
