---
status: implemented
tags: hemera, roadmap
crystal-type: entity
crystal-domain: crypto
---
# async streaming — O(log n) memory verified decode

an async finite state machine that reads a combined pre-order stream from any `AsyncReadExt` source, yielding verified 4 KB chunks one at a time. memory usage is O(tree depth) regardless of content size.

## motivation

the synchronous `stream::decode()` buffers the entire encoded stream in memory before returning. for large files over network connections, this defeats the purpose of streaming. the async decoder reads from the wire, verifies each chunk, and yields it immediately — the full file is never held in memory.

## state machine

```
ReadHeader → [single chunk?] → ReadSingleChunk → Done
                    ↓ no
             InitStack(root_hash, n)
                    ↓
             ProcessStack:
               pop entry (offset, count, is_root, expected_hash)
               ├── count == 1 → ReadLeaf → yield Chunk { offset, data }
               └── count > 1  → ReadParent → push right, push left → loop
                    ↓
             stack empty → Done
```

the stack is a `Vec<(u64, u64, bool, Hash)>` — bounded by tree depth. for 2^64 chunks (the maximum representable), the stack holds at most 64 entries.

## API

```rust
let mut decoder = StreamDecoder::new(root_hash, data_len, reader);
loop {
    match decoder.next().await {
        StreamItem::Chunk { offset, data } => { /* write verified chunk */ }
        StreamItem::Done => break,
        StreamItem::Error(e) => { /* handle */ }
    }
}
```

### progress tracking

```rust
decoder.progress()     // bytes verified and yielded so far
decoder.total()        // total expected bytes
decoder.is_done()      // whether decoding is complete
decoder.into_reader()  // consume decoder, recover the reader
```

## error reporting

```rust
enum StreamError {
    HashMismatch { offset: u64 },  // byte offset of corrupted chunk
    Truncated,                      // stream ended early
    Io(std::io::Error),            // underlying reader error
}
```

`HashMismatch` includes the exact byte offset of the corrupted chunk — useful for debugging partial downloads or network corruption.

## async encoder

`encode_stream` writes a combined stream from an async reader to an async writer:

```rust
let root = encode_stream(data_len, reader, writer).await?;
```

## memory budget

| component | size | bound |
|-----------|------|-------|
| hash stack | 64 × (8+8+1+32) bytes | log₂(max_chunks) entries |
| read buffer | 4096 bytes | one CHUNK_SIZE |
| hash pair buffer | 64 bytes | one PAIR_SIZE |
| total | ~3.5 KB | constant for any file size |

a 1 TB file and a 1 KB file use the same memory.

## feature gate

async streaming requires the `async` feature:

```toml
cyber-hemera = { version = "0.3", features = ["async"] }
```

this pulls in `tokio` as a dependency. the core crate stays `no_std` without this feature.

## implementation

- `rs/src/stream_async.rs` — `StreamDecoder`, `StreamItem`, `StreamError`, `encode_stream`
- feature-gated: `#[cfg(feature = "async")]`

see [[verified-streaming]] for the wire format and synchronous API
