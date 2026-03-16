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
pub fn prove_batch(data: &[u8], indices: &[u64]) -> (Hash, BatchInclusionProof);
pub fn verify_batch(chunks: &[&[u8]], proof: &BatchInclusionProof) -> bool;

// ── Sparse Tree API ────────────────────────────────────────────
pub fn sparse_new(depth: u32) -> SparseTree;
pub fn sparse_insert(tree: &mut SparseTree, key: &[u8; 32], value: &[u8]) -> Hash;
pub fn sparse_delete(tree: &mut SparseTree, key: &[u8; 32]) -> Hash;
pub fn sparse_get(tree: &SparseTree, key: &[u8; 32]) -> Option<&[u8]>;
pub fn sparse_root(tree: &SparseTree) -> Hash;
pub fn sparse_prove(tree: &SparseTree, key: &[u8; 32]) -> CompressedSparseProof;
pub fn sparse_prove_batch(tree: &SparseTree, keys: &[&[u8; 32]]) -> CompressedSparseBatchProof;
pub fn sparse_verify(proof: &CompressedSparseProof, value: Option<&[u8]>, root: &Hash) -> bool;
pub fn sparse_verify_batch(proof: &CompressedSparseBatchProof, values: &[Option<&[u8]>], root: &Hash) -> bool;

// ── Convenience ──────────────────────────────────────────────
pub fn hash(data: &[u8]) -> Hash;
pub fn keyed_hash(key: &[u8; 64], data: &[u8]) -> Hash;

// ── Key derivation ────────────────────────────────────────────
pub fn derive_key(context: &str, key_material: &[u8]) -> [u8; 64];

// ── Output type ───────────────────────────────────────────────
pub struct Hash([u8; 64]);  // 8 Goldilocks elements, LE canonical
```
