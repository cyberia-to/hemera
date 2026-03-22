---
tags: cyber, cip
crystal-type: entity
crystal-domain: cyber
alias: Hemera API, public API
diffusion: 0.00010722364868599256
springs: 0.00007019991600688145
heat: 0.00003419142694206788
focus: 0.00008151008453347325
gravity: 0
density: 0
---

# public API

Hemera — the complete hash primitive for cyber/core.
One sponge. No compression mode. Structured capacity for tree binding.

```rust
// ── Sponge API ────────────────────────────────────────────────
pub struct Hasher { /* sponge state + buffer */ }

impl Hasher {
    pub fn new() -> Self;                           // domain_tag = 0x00
    pub fn new_keyed(key: &[u8; 64]) -> Self;       // domain_tag = 0x01
    pub fn update(&mut self, data: &[u8]) -> &mut Self;
    pub fn finalize(&self) -> Hash;                 // squeeze 8 elements = 64 bytes
    pub fn finalize_xof(&self) -> OutputReader;     // extendable output
}

// ── Tree API ─────────────────────────────────────────────────
pub fn hash_leaf(data: &[u8], counter: u64, is_root: bool) -> Hash;
pub fn hash_node(left: &Hash, right: &Hash, is_root: bool) -> Hash;
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
pub fn keyed_hash(key: &[u8; 64], data: &[u8]) -> Hash;

// ── Key derivation ────────────────────────────────────────────
pub fn derive_key(context: &str, key_material: &[u8]) -> [u8; 64];

// ── Output type ───────────────────────────────────────────────
pub struct Hash([u8; 64]);  // 8 Goldilocks elements, LE canonical
```