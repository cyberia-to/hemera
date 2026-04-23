---
status: implemented
tags: hemera, roadmap
crystal-type: entity
crystal-domain: crypto
---
# zero-alloc core — no_std, fixed-size, embedded-ready

the core `cyber-hemera` crate is `#![no_std]` with zero heap allocation in the hashing path. the Hasher, permutation, field arithmetic, and sponge all operate on fixed-size stack-resident buffers. suitable for embedded systems, WASM targets, and environments without an allocator.

## what allocates and what does not

### zero-alloc (stack only)

| component | buffer | size |
|-----------|--------|------|
| `Hasher` state | `[Goldilocks; 16]` | 128 bytes |
| `Hasher` input buffer | `[u8; RATE_BYTES]` | 56 bytes |
| permutation | in-place mutation of state array | 0 extra |
| field arithmetic | u64 registers | 0 extra |
| `hash()` | Hasher on stack | ~200 bytes total |
| `keyed_hash()` | Hasher on stack | ~200 bytes total |
| `derive_key()` | two Hashers on stack | ~400 bytes total |
| `OutputReader` (XOF) | state array | 128 bytes |

### heap-using (behind features or in tree/stream modules)

| component | allocation | reason |
|-----------|-----------|--------|
| `stream::encode()` | `Vec<u8>` | output size is data-dependent |
| `stream::decode()` | `Vec<u8>` | recovered data buffer |
| `SparseTree` | `BTreeMap` | sparse structure requires dynamic storage |
| `BatchInclusionProof` | `Vec<Hash>` | proof size varies |
| `sentinel_table()` | `Vec<Hash>` | one-time per tree depth |
| `StreamDecoder` (async) | `Vec<(...)>` stack | bounded by tree depth |

heap-using modules import from `alloc` — they work on `no_std` targets that provide a global allocator.

## feature gates

```toml
[features]
default = ["std"]
std = []
serde = ["dep:serde"]
async = ["dep:tokio", "std"]
```

- **default (`std`)**: enables standard library. the core hash API works identically without it.
- **`serde`**: adds Serialize/Deserialize for Hash, InclusionProof, Sibling, NodeIndex.
- **`async`**: enables `stream_async` module. requires `std` (tokio dependency).

a bare `no_std` build: `cyber-hemera = { version = "0.3", default-features = false }`

## rs-edition compliance

the crate passes `rsc --rs-edition` restrictions (RS501-RS507) for the core hashing path:

- no `Box::new()` (RS501) — all values on stack
- no `Vec<T>` in core path (RS502) — fixed-size arrays
- no `String` (RS503) — `&str` only
- no `dyn Trait` (RS504) — generics throughout
- no `Arc`/`Rc` (RS505) — no shared ownership
- no `panic!()` in library (RS506) — `Result` for fallible ops
- no `HashMap` (RS507) — `BTreeMap` where maps are needed

tree and stream modules use `#[allow(rs_no_vec)]` annotations where `Vec` is unavoidable (output buffers, proof data).

## why this matters

1. **embedded**: hemera can run on microcontrollers and bare-metal targets that lack a heap allocator
2. **WASM**: no-std + fixed-size buffers produce small WASM binaries with predictable memory
3. **determinism**: fixed-size buffers eliminate allocation-dependent behavior
4. **proof circuits**: the zero-alloc structure maps directly to circuit constraints — each field operation is a bounded, traceable step
5. **rs-edition**: the crate serves as a reference for rs-lang compliant library design

## implementation

- `rs/src/lib.rs` — `#![no_std]`, `extern crate alloc`
- `rs/src/sponge.rs` — `Hasher` with fixed-size state and buffer
- `rs/src/field.rs` — inline Goldilocks arithmetic
- `rs/src/permutation.rs` — in-place permutation on `&mut [Goldilocks; WIDTH]`

see [[inversion-sbox]] for the permutation design, [[compact-output]] for 32-byte output
