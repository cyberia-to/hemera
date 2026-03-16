---
tags: cyber, cip
crystal-type: entity
crystal-domain: cyber
alias: Hemera tree, tree hashing specification, canonical tree hashing
---

# canonical tree hashing

## Chunk Size: 4 KB (4096 bytes)

Content is split into fixed 4 KB chunks (4096 bytes). Each chunk is hashed via `hash_leaf(data, counter, is_root)`. At 56 input bytes per rate block, one 4 KB chunk requires ⌈4096/56⌉ = 74 absorptions. The last chunk may be shorter. No content-defined chunking — every boundary is deterministic.

See `docs/explanation/chunk-size.md` for rationale.

## Leaf Hashing: hash_leaf(data, counter, is_root) → Hash

Two-pass construction separates content hashing from structural binding:

```
Pass 1 — content hashing:
    hasher ← Hasher::new()              (plain sponge, domain_tag = 0x00)
    hasher.absorb(data)
    base_hash ← hasher.finalize()       (64-byte sponge output)

Pass 2 — structural binding:
    state ← [0; 16]
    state[0..8] ← bytes_to_elements(base_hash)    (8 Goldilocks elements)
    state[8]    ← counter                          (chunk position, 0-based)
    state[9]    ← FLAG_CHUNK | (FLAG_ROOT if is_root)
    state ← permute(state)
    output ← elements_to_bytes(state[0..8])        (64-byte chaining value)
```

Two passes exist for three reasons. The sponge remains a pure hash — no tree metadata contaminates its input stream. Tree logic is layered on top via the capacity region. The `base_hash` is cacheable: if a chunk appears at multiple positions, the expensive 74-absorption pass runs once and the cheap 1-permutation binding runs per position.

Counter prevents chunk reordering. CHUNK flag prevents leaf/node confusion. ROOT flag distinguishes root finalization from subtree chaining.

Cost: N absorptions + 1 permutation. At 4096 bytes: 74 + 1 = 75 permutations per leaf.

## Internal Node Hashing: hash_node(left, right, is_root) → Hash

```
state ← [0; 16]
state[9] ← FLAG_PARENT | (FLAG_ROOT if is_root)

// Absorb left child (8 elements = one full rate block)
state[0..8] += bytes_to_elements(left)     (field addition, element-wise)
state ← permute(state)

// Absorb right child (8 elements = one full rate block)
state[0..8] += bytes_to_elements(right)    (field addition, element-wise)
state ← permute(state)

output ← elements_to_bytes(state[0..8])    (64-byte chaining value)
```

No padding needed — inputs are always exactly two 64-byte hashes. PARENT flag lives in the capacity region. Order matters: `hash_node(A, B) ≠ hash_node(B, A)`.

Cost: 2 permutations per internal node.

## Namespace-Aware Parent: hash_node_nmt(left, right, ns_min, ns_max, is_root) → Hash

Same as `hash_node` but with namespace bounds in the capacity:

```
state[12] ← ns_min        (minimum namespace in subtree)
state[13] ← ns_max        (maximum namespace in subtree)
```

When `ns_min = ns_max = 0`, reduces to `hash_node`. Only NMT uses non-zero namespace bounds.

Verifier checks: `parent.ns_min ≤ left.ns_max < right.ns_min ≤ parent.ns_max` (for sorted NMT).

Cost: 2 permutations (same as `hash_node`).

## Tree Shape

Binary, left-balanced, in-order indexed:

```
If N = 1:     hash_leaf(data, 0, is_root=true)  is the root
If N > 1:     split = 2^(⌈log₂(N)⌉ - 1)
              left  = tree_hash(chunks[0..split],       is_root=false)
              right = tree_hash(chunks[split..N],        is_root=false)
              root  = hash_node(left, right, is_root=true)
```

Left-balanced: the left subtree is always a complete binary tree (power-of-2 leaves). The right subtree absorbs the remainder and recurses with the same rule.

In-order indexing: Leaves at even positions (0, 2, 4, ...). Parents at odd positions with `level = trailing_ones(index)`. This indexing is compatible with Merkle Mountain Range append operations.

## Root Finalization

| Case | Function | Flags | Value |
|---|---|---|---|
| Single-chunk file | `hash_leaf(data, 0, is_root=true)` | CHUNK \| ROOT | 0x05 |
| Multi-chunk file | `hash_node(left, right, is_root=true)` | PARENT \| ROOT | 0x03 |

The ROOT flag ensures that a file's identity hash is never equal to any internal chaining value. A subtree hash with `is_root=false` is structurally distinct from the same tree computed as a standalone file with `is_root=true`.

## Security Properties

| Attack | Defense | Mechanism |
|---|---|---|
| Leaf/node confusion | Prevented | CHUNK (0x04) vs PARENT (0x02) in capacity |
| Chunk reordering | Prevented | Counter in state[8] binds position |
| Chunk duplication | Prevented | Counter distinguishes identical chunks at different offsets |
| Subtree substitution | Prevented | ROOT flag separates file identity from subtree identity |
| Length extension | Prevented | Length in state[10] (sponge) + counter (binding) |
| Second preimage via tree | Prevented | All of the above, combined |

## Complete Example

A 12 KB file (3 chunks):

```
Input:  12,288 bytes → 3 chunks of 4096 bytes each

Chunk 0: hash_leaf(data[0..4096],     counter=0, is_root=false)  → L0
Chunk 1: hash_leaf(data[4096..8192],  counter=1, is_root=false)  → L1
Chunk 2: hash_leaf(data[8192..12288], counter=2, is_root=false)  → L2

Tree:              root
                  /    \
            parent01    L2
            /    \
          L0      L1

split(3) = 2^(⌈log₂(3)⌉ - 1) = 2^(2-1) = 2
  left  = tree_hash([L0, L1], is_root=false)
  right = L2  (single leaf, is_root=false)

parent01 = hash_node(L0, L1, is_root=false)     flags = PARENT       = 0x02
root     = hash_node(parent01, L2, is_root=true) flags = PARENT|ROOT  = 0x03

Cost:  3 leaves  × 75 = 225 permutations
       2 parents × 2  =   4 permutations
       total           = 229 permutations
```

## Performance

Cost breakdown for a file of size S bytes:

```
Leaves:   ⌈S / 4096⌉ chunks × 75 = N × 75 permutations
Parents:  (N − 1) internal nodes × 2 = 2(N − 1) permutations
Total:    75N + 2(N − 1) ≈ 77N permutations
```

The tree overhead is negligible: 2 permutations per parent vs 75 per leaf. Internal nodes add less than 3% to the total cost.

Incremental update: modifying one chunk requires rehashing that leaf (75 permutations) plus the path from leaf to root (⌈log₂(N)⌉ nodes × 2 permutations each). For a 1 GB file (262,144 chunks): 75 + 2 × 18 = 111 permutations to update any single chunk.

## The Universal Tree Primitive

`hash_node` is one function that enters four independent tree structures, each serving a different role in the stack:

| Tree type | Structure | Role | Where |
|---|---|---|---|
| Content tree | Binary Merkle | File addressing (particle hash) | Particle addressing layer |
| MMR | Merkle Mountain Range | Append-only commitment list (AOCL) | BBG Layer 4 |
| NMT | Namespace Merkle Tree | Data availability sampling | DA layer |
| WHIR commitment | Binary Merkle | Polynomial commitment ([[WHIR]]) | Proof system |

The relationship:

```
                        hash_node(left, right, is_root)
                                    │
              ┌─────────────┬───────┴───────┬──────────────┐
              │             │               │              │
         Content tree      MMR             NMT       WHIR commit
         (file hash)    (append log)    (DA proofs)  (poly commit)
              │             │               │              │
              └─────────────┴───────┬───────┴──────────────┘
                                    │
                            Same permutation
                            Same capacity layout
                            Same security proof
```

All four tree types share the same internal node construction. The content tree and WHIR commitment use `hash_node` directly. The MMR uses `hash_node` with `is_root=true` at each peak. The NMT uses `hash_node_nmt` with namespace bounds in state[12..13].

Constraint cost in a STARK circuit:

| Operation | Permutations | Constraints (≈1200/perm) |
|---|---|---|
| hash_leaf (4 KB chunk) | 75 | 90,000 |
| hash_node (internal) | 2 | 2,400 |
| Merkle proof (depth d) | 2d | 2,400d |
| 1 GB file tree root | ≈20M | ≈24B |

One primitive, one circuit, one security analysis. Every tree in the stack benefits from the same audit, the same optimization, and the same hardware acceleration.

## Batch Proofs

A single-leaf inclusion proof carries d sibling hashes (one per tree level). When proving k leaves independently, the total proof contains k × d hashes — but many of those siblings overlap. Batch proofs deduplicate shared path segments into one compact structure.

### Structure: BatchInclusionProof

A batch proof for leaf indices {i₁, i₂, ..., iₖ} against root R contains only the minimal set of hashes that a verifier cannot recompute from the leaf data itself.

```
BatchInclusionProof {
    indices:  [u64; k],          // sorted leaf indices
    siblings: [Hash; m],         // deduplicated sibling hashes, m ≤ k × d
    root:     Hash,              // expected root
}
```

### Construction: prove_batch

Given a tree with N leaves and a set of leaf indices S = {i₁, ..., iₖ} (sorted):

```
prove_batch(data, indices) → (root, BatchInclusionProof):

    tree ← build_tree(data)
    needed ← {}                 // set of internal node positions

    // Walk from each leaf to root, collect nodes the verifier needs
    for idx in indices:
        pos ← leaf_position(idx)
        while pos ≠ root_position:
            sibling ← sibling(pos)
            parent  ← parent(pos)

            // The verifier can compute a node if both its children
            // are already known (either as queried leaves or as
            // parents of queried leaves). Only include the sibling
            // if it is NOT computable from other queried paths.
            if sibling ∉ computable_set:
                needed ← needed ∪ {sibling}

            computable_set ← computable_set ∪ {parent}
            pos ← parent

    siblings ← [tree[pos] for pos in needed], ordered bottom-up left-to-right
    return (tree.root, BatchInclusionProof { indices, siblings, root })
```

The key invariant: a node is computable if both its children are computable. Queried leaves are computable by definition (the verifier has the chunk data). The algorithm propagates computability upward and only emits sibling hashes that cannot be derived.

### Verification: verify_batch

```
verify_batch(chunks, proof) → bool:

    known ← {}    // map from tree position → hash

    // Insert queried leaves
    for (idx, chunk) in zip(proof.indices, chunks):
        known[leaf_position(idx)] ← hash_leaf(chunk, idx, is_root=(N=1))

    // Process siblings bottom-up left-to-right
    sibling_cursor ← 0
    for level in 0..depth:
        for each parent at this level where at least one child is known:
            left_pos  ← left_child(parent)
            right_pos ← right_child(parent)

            left  ← known[left_pos]  or proof.siblings[sibling_cursor++]
            right ← known[right_pos] or proof.siblings[sibling_cursor++]

            is_root ← (parent = root_position)
            known[parent] ← hash_node(left, right, is_root)

    return known[root_position] = proof.root
        AND sibling_cursor = len(proof.siblings)    // all siblings consumed
```

The verifier recomputes every node it can from known children, pulls the next sibling from the proof only when a child is missing. If any sibling remains unconsumed or the reconstructed root mismatches, verification fails.

### Size Analysis

For a tree of depth d with k queried leaves:

| Pattern | Individual proofs | Batch proof | Savings |
|---|---|---|---|
| k = 1 | d hashes | d hashes | 0% (identical) |
| k = 2, adjacent | 2d hashes | d + 1 hashes | ~50% |
| k = 2, distant | 2d hashes | 2d − shared hashes | depends on distance |
| k contiguous | kd hashes | d + k − 1 hashes | ~(k−1)/k × 100% |
| k = N (all leaves) | Nd hashes | 0 hashes | 100% (verifier rebuilds tree) |

Worst case (all leaves maximally spread): m = k × d − (k − 1) siblings. Typical case with locality: m ≈ d + k.

At 64 bytes per hash, proving 32 contiguous chunks from a 2^20-leaf tree: individual proofs = 32 × 20 × 64 = 40,960 bytes. Batch proof = (20 + 31) × 64 = 3,264 bytes. 12.5× reduction.

### Verification Cost

| Operation | Individual (k proofs) | Batch |
|---|---|---|
| `hash_node` calls | k × d | N_internal (nodes on paths, deduplicated) |
| Permutations | 2kd | 2 × N_internal |

For k contiguous leaves: N_internal ≈ d + k − 1. The batch verifier calls `hash_node` once per unique internal node instead of once per proof per level.

In a STARK circuit: batch verification of 32 contiguous chunks costs (20 + 31) × 2 × 1,200 = 122,400 constraints. Individual verification costs 32 × 20 × 2 × 1,200 = 1,536,000 constraints. Same 12.5× reduction in proving cost.

### API

```rust
pub fn prove_batch(data: &[u8], indices: &[u64]) -> (Hash, BatchInclusionProof);
pub fn verify_batch(chunks: &[&[u8]], proof: &BatchInclusionProof) -> bool;
```

`prove_batch` with a single index produces the same sibling set as `prove`. `prove_range(start, end)` is equivalent to `prove_batch(data, &(start..end).collect())`.
