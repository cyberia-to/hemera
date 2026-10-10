---
tags: cyber, cip
crystal-type: entity
crystal-domain: cyber
alias: Hemera API, public API
---

# public API

Hemera — the complete hash primitive for cyber/core.
One sponge. No compression mode. Structured capacity for tree binding.

```rust
// ── Sponge API ────────────────────────────────────────────────
pub struct Hasher { /* sponge state + buffer */ }

impl Hasher {
    pub fn new() -> Self;                           // domain_tag = 0x00
    pub fn new_secret() -> Self;                    // domain_tag = 0x00, constant time
    pub fn new_keyed(key: &[u8; 32]) -> Self;       // domain_tag = 0x01
    pub fn update(&mut self, data: &[u8]) -> &mut Self;
    pub fn finalize(&self) -> Hash;                 // squeeze 4 elements = 32 bytes
    pub fn finalize_xof(&self) -> OutputReader;     // extendable output
}

// ── Tree API ─────────────────────────────────────────────────
pub fn hash_leaf(data: &[u8], counter: u64, is_root: bool) -> Hash;
pub fn hash_node(left: &Hash, right: &Hash, is_root: bool) -> Hash;
pub fn hash_leaf_batch(leaves: &[(&[u8], u64)], is_root: bool, out: &mut [Hash]);
pub fn hash_node_batch(pairs: &[(Hash, Hash)], is_root: bool, out: &mut [Hash]);
pub fn hash_node_nmt(left: &Hash, right: &Hash, ns_min: u64, ns_max: u64, is_root: bool) -> Hash;
pub fn root_hash(data: &[u8]) -> Hash;
pub fn build_tree(data: &[u8]) -> TreeNode;
pub fn prove(data: &[u8], chunk_index: u64) -> (Hash, InclusionProof);
pub fn prove_range(data: &[u8], start: u64, end: u64) -> (Hash, InclusionProof);
pub fn verify_proof(chunk_data: &[u8], proof: &InclusionProof, root: &Hash) -> bool;
pub fn verify_node_proof(node_hash: &Hash, proof: &InclusionProof, root: &Hash) -> bool;

// ── Batch Proof API ──────────────────────────────────────────
pub fn prove_batch(data: &[u8], indices: &[u64]) -> (Hash, BatchInclusionProof);
pub fn verify_batch(chunks: &[&[u8]], proof: &BatchInclusionProof) -> bool;

// ── Sparse Tree API ────────────────────────────────────────────
impl SparseTree {
    pub fn new(depth: u32) -> Self;
    pub fn new_default() -> Self;                       // depth = 256
    pub fn root(&self) -> Hash;
    pub fn get(&self, key: &[u8; 32]) -> Option<&[u8]>;
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
    pub fn insert(&mut self, key: &[u8; 32], value: &[u8]) -> Hash;
    pub fn delete(&mut self, key: &[u8; 32]) -> Hash;
    pub fn prove(&self, key: &[u8; 32]) -> CompressedSparseProof;
    pub fn verify(proof: &CompressedSparseProof, value: Option<&[u8]>,
                  root: &Hash, depth: u32) -> bool;
}

// ── Convenience ──────────────────────────────────────────────
pub fn hash(data: &[u8]) -> Hash;
pub fn hash_secret(data: &[u8]) -> Hash;          // = hash(data), constant time
pub fn keyed_hash(key: &[u8; 32], data: &[u8]) -> Hash;

// ── Key derivation ────────────────────────────────────────────
pub fn derive_key(context: &str, key_material: &[u8]) -> [u8; 32];

// ── Permutation ───────────────────────────────────────────────
pub fn permute(state: &mut [Goldilocks; 16]);        // output limbs canonical
pub fn permute_ct(state: &mut [Goldilocks; 16]);     // = permute, constant time
pub fn permute_batch(states: &mut [[Goldilocks; 16]]);

// ── Output type ───────────────────────────────────────────────
pub struct Hash([u8; 32]);  // 4 Goldilocks elements, LE canonical
```

## constant time

Every entry point has exactly one of two execution contracts. Both
produce the same bits; they differ only in timing behaviour.

**constant time** — no branch and no memory access depends on the input
bytes, the key, or any intermediate field value; only on input *lengths*
and the public round structure:

| entry point | why |
|---|---|
| `keyed_hash`, `Hasher::new_keyed` | the key is secret |
| `derive_key`, `Hasher::new_derive_key_context`, `Hasher::new_derive_key_material` | key material is secret |
| `hash_secret`, `Hasher::new_secret` | plain-mode digest of secret input (seeds, entropy, private scalars) |
| `OutputReader` from any of the hashers above | inherits the hasher's mode |
| `permute_ct` | building block for other secret-input constructions |

A hasher's mode is fixed by its constructor and cannot be changed, so a
caller of the keyed and derive-key APIs cannot reach the fast kernel.
`update_traced` / `finalize_traced` are witness generation and are not
constant time in any mode.

**fast** — everything else: `hash`, `Hasher::new`, the tree, batch,
sparse, stream and CDC APIs, `permute`, `permute_batch`. These are meant
for public data (content, Merkle nodes and leaves, Fiat–Shamir
transcripts). Their Goldilocks reduction takes a predicted branch when
`lo < hi_hi` (probability ≈ 2⁻³² per multiplication) — a timing signal
about the data. Hashing a secret with `hash` is a misuse; use
`hash_secret` (same digest).

Implementation: the permutation kernel is generic over an arithmetic
backend (`rs/src/backend.rs`); the constant-time backend takes every
reduction fix-up by `csel`/mask (hand-written aarch64 assembly;
mask arithmetic in the portable path), field addition and canonical
reduction are mask-based throughout. Checked bit-for-bit against the fast
kernel on 10⁶ random states (`rs/tests/constant_time.rs`); the compiled
`permute_ct` was inspected for conditional branches (all on loop
counters) — `audit/permutation-performance-2026-10.md` §6.