# hemera

> **WARNING: This is novel, unaudited cryptography. The parameter set, sponge
> construction, and self-bootstrapping round constant generation have not been
> reviewed by third-party cryptographers. Do not use in production systems
> where cryptographic guarantees are required. Use at your own risk.**

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

## Usage

```rust
use hemera::{hash, keyed_hash, derive_key, Hasher};

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

// XOF (extendable output)
let mut xof = Hasher::new().update(b"seed").finalize_xof();
let mut out = [0u8; 256];
xof.fill(&mut out);
```

## Self-bootstrapping round constants

Hemera generates its own round constants from a genesis seed `[0x63, 0x79, 0x62, 0x65, 0x72]` ("cyber") through a zero-constant Poseidon2 sponge. No external PRNG, no nothing-up-my-sleeve numbers from other sources.

## GPU backend

Optional GPU acceleration via wgpu compute shaders:

```toml
hemera = { version = "0.1", features = ["gpu"] }
```

## Features

| Feature | Description |
|---------|-------------|
| `gpu`   | GPU-accelerated batch permutations via wgpu |
| `serde` | Serialize/deserialize `Hash` type |

## License

MIT OR Apache-2.0
