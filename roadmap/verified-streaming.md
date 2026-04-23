---
status: implemented
tags: hemera, roadmap
crystal-type: entity
crystal-domain: crypto
---
# verified streaming — content-verified encode/decode

pre-order tree traversal interleaved with leaf data. a receiver verifies every chunk against the hash tree as it arrives — no need to download the entire file first.

## formats

two complementary layouts:

### combined (pre-order)

```
[8 bytes: data_len as LE u64]
[pre-order traversal of tree]
  parent → left_hash ‖ right_hash   (64 bytes)
  leaf   → raw chunk data            (≤ 4096 bytes)
```

pre-order means parent hashes appear before their children. a decoder reads a parent hash pair, verifies it against the expected hash, then recurses into children. when a leaf is reached, its data is verified and yielded. every byte is authenticated before it reaches the consumer.

### outboard

```
[8 bytes: data_len as LE u64]
[pre-order parent hash pairs only — no leaf data]
```

outboard stores only the hash tree. the original data stays separate. useful when the data is already stored elsewhere (e.g., IPFS, disk) and only integrity metadata is needed.

## API

```rust
// encode data into combined format
let (root, encoded) = stream::encode(data);

// decode and verify combined stream
let decoded = stream::decode(&encoded, &root)?;

// compute outboard (hash tree without data)
let (root, ob) = stream::outboard(data);

// verify data against outboard
stream::verify_outboard(data, &ob, &root)?;
```

## properties

| property | value |
|----------|-------|
| chunk size | 4096 bytes |
| hash pair size | 64 bytes (2 × 32) |
| header | 8 bytes (LE u64 data length) |
| tree shape | left-balanced binary |
| domain separation | leaf vs parent via capacity flags |
| counter binding | leaf chunks include position index |

## incremental verification

the pre-order layout is chosen for streaming. a decoder at any point has verified every byte it has yielded. if a hash mismatch occurs, the decoder stops immediately with an error identifying the corrupted region. no valid data is yielded after corruption.

this is the same design principle as BLAKE3's verified streaming, adapted for hemera's Poseidon2 sponge. the key difference: hemera tree nodes are single-permutation (32-byte children fit in one rate block), so overhead per node is one Poseidon2 call.

## single-chunk optimization

files that fit in one chunk (≤ 4096 bytes) skip the tree entirely. the combined format is just the header followed by raw data. root hash = leaf hash with `is_root = true`.

## overhead

```
combined format size = 8 + (n-1) × 64 + data_len    (n = number of chunks)
outboard size        = 8 + (n-1) × 64
```

for a 1 GB file: ~500 KB of hash overhead (~0.05%). for a 1 KB file: 8 bytes header, no hash pairs.

## implementation

- `rs/src/stream.rs` — synchronous encode, decode, outboard, verify_outboard
- `rs/src/stream_async.rs` — async FSM decoder (see [[async-streaming]])
- CLI: `hemera encode`, `hemera decode`, `hemera outboard`

see [[async-streaming]] for the O(log n) memory async decoder, [[compact-output]] for why 32-byte output enables single-permutation tree nodes
