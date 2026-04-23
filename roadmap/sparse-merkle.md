---
status: implemented
tags: hemera, roadmap
crystal-type: entity
crystal-domain: crypto
---
# sparse Merkle tree — authenticated key-value storage

a fixed-depth Merkle tree over 256-bit keys. only non-empty subtrees are stored. empty subtrees use precomputed sentinel hashes. supports inclusion proofs, non-inclusion proofs, and compressed proof encoding.

## structure

```
depth = 256 (matching key length in bits)
2^256 possible leaf positions
path from root to leaf follows key bits MSB-first
  bit 0 of byte[0] = first branching decision
```

the tree is sparse: most positions are empty. a tree with 1000 entries stores ~1000 leaf hashes and ~256,000 internal nodes — the other 2^256 - 1000 positions are sentinels.

## sentinel hashes

empty subtrees at every level have deterministic hashes:

```
sentinel[0] = hash_leaf(b"", 0, false)         // empty leaf
sentinel[d] = hash_node(sentinel[d-1], sentinel[d-1], d == depth)
```

the sentinel table is computed once per tree instance. since hemera's hash is deterministic, all trees with the same depth share the same sentinels.

## leaf binding

leaf hashes commit both key and value:

```
leaf_hash = structural_bind(Hasher::new().update(key).update(value).finalize())
```

the key is included in the hashed data even though it is already encoded in the tree path. defense in depth: even if the tree structure is somehow corrupted, the key is cryptographically bound to the leaf.

## compressed proofs

standard Merkle proofs for depth-256 trees would require 256 sibling hashes (8 KB). most siblings are sentinels. the compressed format encodes sentinel positions as a bitmask:

```rust
struct CompressedSparseProof {
    key: [u8; 32],       // the key being proved
    bitmask: [u8; 32],   // 256-bit: bit i = 1 means real sibling at level i
    siblings: Vec<Hash>, // only non-sentinel siblings
}
```

a tree with 1000 entries typically has ~10 non-sentinel siblings per proof. proof size: 32 + 32 + 10×32 = 384 bytes, vs 8192 bytes uncompressed.

## API

```rust
// create and populate
let mut tree = SparseTree::new_default();  // depth 256
tree.insert(&key, b"value");
tree.delete(&key);

// query
tree.root()           // current root hash
tree.get(&key)        // lookup value
tree.len()            // populated leaves
tree.is_empty()

// prove and verify
let proof = tree.prove(&key);
SparseTree::verify(&proof, Some(b"value"), &root, 256)  // inclusion
SparseTree::verify(&proof, None, &root, 256)             // non-inclusion
```

## properties

| property | value |
|----------|-------|
| key size | 32 bytes (256 bits) |
| default depth | 256 |
| path encoding | MSB-first |
| deterministic roots | insertion order does not affect root hash |
| deletion | restores empty subtree sentinels, prunes empty branches |
| proof type | inclusion and non-inclusion |
| proof compression | bitmask encoding, ~97% smaller for sparse trees |

## determinism

insertion order does not matter. two trees with the same key-value pairs produce the same root hash regardless of operation sequence. this is verified by tests that insert in different orders and assert root equality.

## interaction with protocol

sparse Merkle trees provide authenticated key-value storage for:

- neuron identity → stake mapping
- nullifier sets (spent record tracking)
- state commitments (key-value state roots)

the tree uses hemera's `hash_node` for internal nodes — same domain separation as the content-hash tree, using capacity flags.

## implementation

- `rs/src/sparse.rs` — `SparseTree`, `CompressedSparseProof`, `sentinel_table()`
- CLI: `hemera sparse new`, `insert`, `get`, `prove`, `verify`, `sentinel`

see [[compact-output]] for why 32-byte hashes keep proof sizes manageable
