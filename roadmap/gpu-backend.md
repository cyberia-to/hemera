---
status: implemented
tags: hemera, roadmap
crystal-type: entity
crystal-domain: crypto
---
# GPU backend — wgpu compute shaders for hemera

full Poseidon2 permutation implemented in WGSL compute shaders. batch operations dispatch thousands of hashes in parallel via wgpu. the GPU backend is optional — CPU is always the fallback.

## challenge: u64 in WGSL

WGSL has no native 64-bit integers. Goldilocks arithmetic (p = 2^64 - 2^32 + 1) requires u64. the solution: emulate u64 as `vec2<u32>` (lo, hi) with custom add, sub, mul, and modular reduction.

```wgsl
// Goldilocks field element = vec2<u32>
fn gl_add(a: vec2<u32>, b: vec2<u32>) -> vec2<u32>
fn gl_sub(a: vec2<u32>, b: vec2<u32>) -> vec2<u32>
fn gl_mul(a: vec2<u32>, b: vec2<u32>) -> vec2<u32>
fn gl_inv(a: vec2<u32>) -> vec2<u32>           // field inversion via Fermat
fn gl_pow7(a: vec2<u32>) -> vec2<u32>           // x⁷ S-box
fn gl_reduce(lo: u32, hi: u32) -> vec2<u32>     // modular reduction
```

multiplication uses the 64×64→128 bit schoolbook method, then reduces mod p using the Goldilocks structure: `2^64 ≡ 2^32 - 1 (mod p)`.

## compute pipelines

| pipeline | entry point | workgroup size | purpose |
|----------|-------------|---------------|---------|
| permute | `hemera_permute` | 64 | raw Poseidon2 permutation |
| hash_leaf | `hemera_hash_leaf` | 64 | sponge + leaf domain flags |
| hash_node | `hemera_hash_node` | 64 | parent node hashing |
| hash_chunk | `hemera_hash_chunk` | 64 | sponge hash (no tree domain) |
| keyed_hash | `hemera_keyed_hash` | 64 | keyed sponge hash |
| derive_key_material | `hemera_derive_key_material` | 64 | key derivation phase 2 |
| hash_node_nmt | `hemera_hash_node_nmt` | 64 | namespace-aware node hashing |

all pipelines share the same bind group layout: input buffer, output buffer, round constants buffer, diagonal buffer, params buffer.

## GPU tree hashing

`root_hash()` computes a full Merkle tree on GPU:

1. dispatch `hash_leaf` for all chunks in parallel → leaf hashes
2. iteratively dispatch `hash_node` to merge pairs → level by level
3. final single hash = root

for a 1 GB file (~262,144 chunks): 18 GPU dispatches (1 leaf + 17 merge levels).

## batch operations

```rust
let gpu = GpuContext::new().await?;

// batch hash — thousands of chunks in one dispatch
let hashes = gpu.batch_hash(&chunks).await;

// batch keyed hash
let macs = gpu.batch_keyed_hash(&key, &chunks).await;

// batch key derivation
let keys = gpu.batch_derive_key("context", &materials).await;

// batch proof verification
let results = gpu.batch_verify_proofs(&proofs).await;

// batch XOF squeeze
let outputs = gpu.batch_squeeze(&root, bytes_per, count).await;
```

## outboard on GPU

`outboard()` uses the GPU for leaf hashing (the parallelizable part), then builds the tree structure on CPU:

```rust
let (root, outboard_bytes) = gpu.outboard(data).await;
```

## progress reporting

`root_hash_with_progress` reports completion percentage:

```rust
let root = gpu.root_hash_with_progress(data, &|done, total| {
    println!("{:.1}%", done as f64 / total as f64 * 100.0);
}).await;
```

## shader modules

| file | responsibility |
|------|---------------|
| `params.wgsl` | constants — field, sponge, tree parameters, capacity indices |
| `field.wgsl` | Goldilocks arithmetic — u64 emulation via vec2<u32> |
| `encoding.wgsl` | bytes ↔ field element conversion (7-byte chunks) |
| `permutation.wgsl` | Poseidon2 permutation — full/partial rounds, S-box dispatch |
| `sponge.wgsl` | sponge hash, keyed hash, derive-key material |
| `tree.wgsl` | leaf/node hashing with domain flags |
| `entry_points.wgsl` | workgroup dispatch for all operations |

## fallback behavior

`GpuContext::new()` returns `None` if no compute-capable GPU adapter is found. the CLI detects this and falls back to CPU. both backends produce identical output — cross-verified by 45 tests.

## implementation

- `wgsl/src/lib.rs` — `GpuContext`, all batch methods
- `wgsl/src/shaders/` — WGSL compute shaders
- `wgsl/tests/` — 39 GPU integration tests
- CLI: `--gpu` / `--cpu` flags

see [[inversion-sbox]] for the x⁻¹ partial S-box implemented in WGSL
