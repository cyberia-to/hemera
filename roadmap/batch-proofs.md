---
status: implemented
tags: hemera, roadmap
crystal-type: entity
crystal-domain: crypto
---
# batch inclusion proofs — deduplicated multi-leaf verification

prove multiple leaves in a single proof. siblings shared across individual proofs are emitted once. proof size scales with the number of unique subtrees, not the number of leaves.

## motivation

verifying k leaves individually requires k separate proofs, each with O(log n) siblings. many siblings overlap — adjacent leaves share parents, nearby leaves share higher ancestors. a batch proof deduplicates these overlaps.

## construction

the prover walks the tree recursively. at each node:

- if both subtrees contain targets: recurse into both, emit no sibling
- if only one subtree contains targets: emit the other subtree's hash as a sibling, recurse into the target subtree
- if neither subtree contains targets: return the precomputed hash

siblings are stored in depth-first left-to-right order. the verifier consumes them in the same order — deterministic, no index metadata needed.

## proof structure

```rust
struct BatchInclusionProof {
    indices: Vec<u64>,     // sorted leaf chunk indices
    siblings: Vec<Hash>,   // deduplicated sibling hashes
    num_chunks: u64,       // total chunks in tree
    root: Hash,            // expected root hash
}
```

## savings

```
1024 chunks, 32 contiguous leaves:
  individual: 32 × 10 = 320 siblings
  batch:      ~10 siblings (log₂(1024) for the boundary)
  compression: ~97%

1024 chunks, 2 distant leaves (indices 0 and 1023):
  individual: 2 × 10 = 20 siblings
  batch:      ~18 siblings (most of the tree is needed)
  compression: ~10%

n chunks, all n leaves:
  individual: n × log₂(n) siblings
  batch:      0 siblings (verifier can recompute everything)
```

the savings depend on spatial locality. contiguous ranges compress well. scattered leaves compress less. all-leaves proofs need zero siblings.

## API

```rust
// prove
let (root, proof) = batch::prove_batch(data, &[0, 1, 5, 7]);

// verify
let chunks = [&data[0..4096], &data[4096..8192], /* ... */];
assert!(batch::verify_batch(&chunks, &proof));
```

## verification

the verifier performs the same recursive tree walk as the prover. at each step, it either:

- computes a leaf hash from provided chunk data
- consumes the next sibling hash from the proof
- recurses into both children

verification succeeds if and only if the reconstructed root matches `proof.root` and all siblings are consumed (no leftover data).

## properties

| property | value |
|----------|-------|
| indices | must be sorted, unique, in range |
| sibling ordering | depth-first left-to-right |
| single-chunk files | proof has zero siblings |
| wrong data | verification fails immediately |
| extra siblings | rejected (cursor must be fully consumed) |

## implementation

- `rs/src/batch.rs` — `prove_batch()`, `verify_batch()`, `BatchInclusionProof`
- CLI: `hemera prove-batch`, `hemera verify-batch`

see [[verified-streaming]] for single-leaf streaming verification, [[sparse-merkle]] for key-value proofs
