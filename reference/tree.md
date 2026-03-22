---
tags: cyber, cip
crystal-type: entity
crystal-domain: cyber
alias: Hemera tree, tree hashing specification, canonical tree hashing
diffusion: 0.00010722364868599256
springs: 0.00007019991600688145
heat: 0.00003419142694206788
focus: 0.00008151008453347325
gravity: 0
density: 0
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

Construction uses a recursive tree walk. At each node in the left-balanced tree, the algorithm determines which subtrees contain target leaves. If both subtrees have targets, it recurses into both and emits no sibling. If only one subtree has targets, it computes the other subtree's hash and emits it as a sibling, then recurses into the target side.

```
prove_batch(data, indices) → (root, BatchInclusionProof):

    collect_siblings(data, offset, count, targets, is_root, siblings):
        local ← targets that fall in [offset, offset+count)

        if local is empty:
            return merge_range(data, offset, count, is_root)

        if count = 1:
            return hash_leaf(data[offset], offset, is_root=false)

        split ← 2^(⌈log₂(count)⌉ - 1)
        left_has  ← any target in [offset, offset+split)
        right_has ← any target in [offset+split, offset+count)

        if left_has AND right_has:
            left  ← collect_siblings(data, offset, split, targets, false, siblings)
            right ← collect_siblings(data, offset+split, count-split, targets, false, siblings)
        elif left_has:
            right ← merge_range(data, offset+split, count-split, false)
            siblings.push(right)
            left  ← collect_siblings(data, offset, split, targets, false, siblings)
        else:
            left ← merge_range(data, offset, split, false)
            siblings.push(left)
            right ← collect_siblings(data, offset+split, count-split, targets, false, siblings)

        return hash_node(left, right, is_root)

    siblings ← []
    root ← collect_siblings(data, 0, N, indices, true, siblings)
    return (root, BatchInclusionProof { indices, siblings, root })
```

The key invariant: a subtree with targets in both children needs no sibling — the verifier can compute both sides from the provided chunk data. Only subtrees entirely outside the target set appear as siblings. This naturally deduplicates shared paths.

Sibling ordering is depth-first, left-before-right — matching the recursive tree walk. The verifier follows the same recursion and consumes siblings in the same deterministic order.

### Verification: verify_batch

The verifier follows the same recursive tree walk as construction:

```
verify_batch(chunks, proof) → bool:

    verify_subtree(chunks, indices, siblings, cursor, offset, count, is_root):
        local ← indices that fall in [offset, offset+count)

        if local is empty:
            return siblings[cursor++]       // consume next sibling

        if count = 1:
            return hash_leaf(chunks[local_index], offset, is_root=false)

        split ← 2^(⌈log₂(count)⌉ - 1)
        left_has  ← any index in [offset, offset+split)
        right_has ← any index in [offset+split, offset+count)

        if left_has AND right_has:
            left  ← verify_subtree(..., offset, split, false)
            right ← verify_subtree(..., offset+split, count-split, false)
        elif left_has:
            right ← siblings[cursor++]
            left  ← verify_subtree(..., offset, split, false)
        else:
            left  ← siblings[cursor++]
            right ← verify_subtree(..., offset+split, count-split, false)

        return hash_node(left, right, is_root)

    cursor ← 0
    computed_root ← verify_subtree(chunks, proof.indices, proof.siblings, cursor, 0, N, true)
    return computed_root = proof.root AND cursor = len(proof.siblings)
```

The verifier recomputes the root from chunk data and consumed siblings. If any sibling remains unconsumed or the reconstructed root mismatches, verification fails.

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

## Sparse Trees

The content tree and MMR are dense — every leaf position holds data. Sparse trees address a different problem: authenticated key-value storage where most of the keyspace is empty. A sparse Merkle tree has a fixed depth (matching the key length in bits) and 2^d possible leaf positions, almost all of which are empty.

### Sentinel Hashes

An empty subtree has a known hash at every level. These sentinel hashes are precomputed once and never stored in the tree:

```
EMPTY[0] = hash_leaf([], counter=0, is_root=false)    // empty leaf: zero-length data
EMPTY[d] = hash_node(EMPTY[d-1], EMPTY[d-1], is_root=(d = DEPTH))

Precomputed table (DEPTH entries):
    EMPTY[0]  = hash of empty leaf
    EMPTY[1]  = hash_node(EMPTY[0], EMPTY[0], false)
    EMPTY[2]  = hash_node(EMPTY[1], EMPTY[1], false)
    ...
    EMPTY[DEPTH] = root of a completely empty tree
```

The sentinel table has DEPTH entries (e.g. 256 entries for a 256-bit keyspace). Each entry is 64 bytes. Total: 16 KB for a 256-deep tree. Computed once at initialization, reused across all sparse tree operations.

### Key Path

A key K is a 32-byte array. Bits are indexed MSB-first: bit 0 is the most significant bit of byte[0], bit 7 is the least significant bit of byte[0], bit 8 is the most significant bit of byte[1], and so on.

```
key_bit(K, i):
    byte_idx ← i / 8
    bit_in_byte ← 7 - (i % 8)
    return (K[byte_idx] >> bit_in_byte) & 1

path(K, depth):
    for i in 0..depth:
        if key_bit(K, i) = 0: descend left
        if key_bit(K, i) = 1: descend right
```

### Sparse Leaf Hashing

Sparse leaves hash both the key and the value through the sponge, then apply structural binding:

```
sparse_hash_leaf(key, value) → Hash:
    hasher ← Hasher::new()
    hasher.absorb(key)         // 32-byte key committed first
    hasher.absorb(value)       // then the value
    base_hash ← hasher.finalize()

    state ← [0; 16]
    state[0..8] ← bytes_to_elements(base_hash)
    state[8]    ← 0                               // counter = 0 (position encoded in tree path)
    state[9]    ← FLAG_CHUNK
    state ← permute(state)
    output ← elements_to_bytes(state[0..8])
```

The key is committed in the leaf hash (defense in depth alongside the tree path binding). Counter is 0 because position is encoded structurally — the key bits determine the path from root to leaf.

### Representation

Only non-empty subtrees are stored. An internal node whose hash equals the sentinel for its level is not materialized:

```
SparseTree {
    depth:    u32,                                    // fixed, typically 256
    root:     Hash,                                   // current root hash
    nodes:    BTreeMap<(u32, [u8; 32]), Hash>,        // (level, masked_key) → hash
    leaves:   BTreeMap<[u8; 32], Vec<u8>>,            // key → value
    sentinels: [Hash; DEPTH + 1],                     // precomputed sentinel table
}
```

Node keys are masked to the relevant prefix length: at level l (counting from leaf = 0), the masked key retains the top `depth − l` bits and zeros the rest. Two keys sharing a prefix at a given level map to the same internal node.

A tree with n populated leaves stores at most n × depth nodes in the worst case (all paths disjoint). In practice, shared prefixes reduce this to approximately n × (depth − log₂(n)) nodes.

### Insert and Update

```
insert(tree, key, value):
    leaf_hash ← sparse_hash_leaf(key, value)
    tree.leaves[key] ← value
    tree.nodes[(0, mask_key(key, depth))] ← leaf_hash

    // Walk from leaf to root, recomputing hashes
    current ← leaf_hash
    for level in 0..depth:
        bit_pos ← depth - 1 - level
        is_right ← key_bit(key, bit_pos)
        sibling ← sibling_hash(tree, key, level)    // sentinel if absent

        if is_right:
            parent ← hash_node(sibling, current, is_root=(level+1 = depth))
        else:
            parent ← hash_node(current, sibling, is_root=(level+1 = depth))

        tree.nodes[(level+1, mask_key(key, depth-level-1))] ← parent
        current ← parent

    tree.root ← current
```

`sibling_hash` flips the bit at position `bit_pos` in the key, masks to the prefix length for that level, and looks up the node hash (returning the sentinel if absent).

Cost: depth × 2 permutations (one `hash_node` per level) + 1 `sparse_hash_leaf`. For depth 256: 512 + 75 permutations per insert.

### Delete

```
delete(tree, key):
    tree.leaves.remove(key)
    tree.nodes.remove((0, mask_key(key, depth)))

    // Replace leaf with empty sentinel, recompute path
    current ← EMPTY[0]
    for level in 0..depth:
        bit_pos ← depth - 1 - level
        is_right ← key_bit(key, bit_pos)
        sibling ← sibling_hash(tree, key, level)

        // If both children are sentinels, parent is sentinel — prune
        if current = EMPTY[level] AND sibling = EMPTY[level]:
            tree.nodes.remove((level+1, mask_key(key, depth-level-1)))
            current ← EMPTY[level+1]
        else:
            if is_right:
                parent ← hash_node(sibling, current, is_root=(level+1 = depth))
            else:
                parent ← hash_node(current, sibling, is_root=(level+1 = depth))
            tree.nodes[(level+1, mask_key(key, depth-level-1))] ← parent
            current ← parent

    tree.root ← current
```

Pruning: when both children of a node are empty sentinels, the node itself is the sentinel for its level. The algorithm removes pruned nodes from storage, keeping the tree compact.

### Inclusion Proof

Proves that key K maps to value V:

```
prove_sparse(tree, key) → CompressedSparseProof:
    bitmask ← [0; 32]         // 256 bits
    siblings ← []
    for level in 0..depth:
        sibling ← sibling_hash(tree, key, level)
        if sibling ≠ EMPTY[level]:
            bitmask[level/8] |= 1 << (level % 8)
            siblings.push(sibling)
    return CompressedSparseProof { key, bitmask, siblings }
```

Verification walks from `sparse_hash_leaf(key, value)` to root, reconstructing the path. At each level, the verifier reads the bitmask: bit 0 means use `EMPTY[level]`, bit 1 means consume the next sibling from the proof.

```
verify_sparse(proof, value, root, depth) → bool:
    current ← sparse_hash_leaf(proof.key, value)    // or EMPTY[0] if value is None
    cursor ← 0
    for level in 0..depth:
        has_real ← (proof.bitmask[level/8] >> (level%8)) & 1
        sibling ← if has_real: proof.siblings[cursor++] else: EMPTY[level]
        bit_pos ← depth - 1 - level
        is_right ← key_bit(proof.key, bit_pos)
        is_root ← (level+1 = depth)
        current ← if is_right: hash_node(sibling, current, is_root)
                  else:        hash_node(current, sibling, is_root)
    return current = root AND cursor = len(proof.siblings)
```

### Non-Inclusion Proof

Proves that key K has no value (maps to empty). The proof structure is identical to an inclusion proof — the verifier computes the path starting from `EMPTY[0]` instead of `sparse_hash_leaf(key, value)` and checks that it reaches the root. If the reconstructed root matches, the key is absent.

No additional mechanism is needed. The sentinel hashes make absence and presence proofs structurally identical.

### Proof Size

All sparse proofs use compression by default. The `CompressedSparseProof` structure replaces sentinel siblings with a bitmask:

```
CompressedSparseProof {
    key:       [u8; 32],
    bitmask:   [u8; 32],          // 256 bits: 1 = real sibling, 0 = sentinel
    siblings:  [Hash; popcount],  // only non-sentinel siblings
}
```

Compressed proof size: 32 bytes (key) + 32 bytes (bitmask) + popcount × 64 bytes. For a tree with n = 1,000 leaves, a typical path has ~10 non-sentinel siblings and ~246 sentinel siblings. Compressed proof: 64 + 10 × 64 = 704 bytes instead of 16,384 bytes (uncompressed). 23× reduction.

### API

```rust
// Sparse tree
impl SparseTree {
    pub fn new(depth: u32) -> Self;
    pub fn root(&self) -> Hash;
    pub fn get(&self, key: &[u8; 32]) -> Option<&[u8]>;
    pub fn len(&self) -> usize;
    pub fn insert(&mut self, key: &[u8; 32], value: &[u8]) -> Hash;
    pub fn delete(&mut self, key: &[u8; 32]) -> Hash;
    pub fn prove(&self, key: &[u8; 32]) -> CompressedSparseProof;
    pub fn verify(proof: &CompressedSparseProof, value: Option<&[u8]>, root: &Hash, depth: u32) -> bool;
}
```

`verify` with `value: None` verifies non-inclusion. With `value: Some(data)` verifies inclusion. The same proof structure serves both cases.