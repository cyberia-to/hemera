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
| Content tree | Binary Merkle | File addressing (particle hash) | Content addressing layer |
| MMR | Merkle Mountain Range | Append-only commitment list (AOCL) | BBG Layer 4 |
| NMT | Namespace Merkle Tree | Data availability sampling | DA layer |
| FRI commitment | Binary Merkle | Polynomial commitment (FRI/WHIR) | Proof system |

The relationship:

```
                        hash_node(left, right, is_root)
                                    │
              ┌─────────────┬───────┴───────┬──────────────┐
              │             │               │              │
         Content tree      MMR             NMT        FRI commit
         (file hash)    (append log)    (DA proofs)   (poly commit)
              │             │               │              │
              └─────────────┴───────┬───────┴──────────────┘
                                    │
                            Same permutation
                            Same capacity layout
                            Same security proof
```

All four tree types share the same internal node construction. The content tree and FRI commitment use `hash_node` directly. The MMR uses `hash_node` with `is_root=true` at each peak. The NMT uses `hash_node_nmt` with namespace bounds in state[12..13].

Constraint cost in a STARK circuit:

| Operation | Permutations | Constraints (≈1200/perm) |
|---|---|---|
| hash_leaf (4 KB chunk) | 75 | 90,000 |
| hash_node (internal) | 2 | 2,400 |
| Merkle proof (depth d) | 2d | 2,400d |
| 1 GB file tree root | ≈20M | ≈24B |

One primitive, one circuit, one security analysis. Every tree in the stack benefits from the same audit, the same optimization, and the same hardware acceleration.
