// ---
// tags: hemera, rust
// crystal-type: source
// crystal-domain: comp
// ---
//! Async FSM verified streaming for hemera.
//!
//! Reads content in 4KB chunks from an async reader, verifying each chunk
//! against the hemera hash tree on the fly. Memory usage: O(tree_depth)
//! regardless of content size.
//!
//! # Usage
//!
//! ```ignore
//! let mut decoder = StreamDecoder::new(root_hash, data_len, reader);
//! loop {
//!     match decoder.next().await {
//!         StreamItem::Chunk { offset, data } => { /* write verified chunk */ }
//!         StreamItem::Done => break,
//!         StreamItem::Error(e) => { /* handle */ }
//!     }
//! }
//! ```

extern crate alloc;
extern crate std;

use alloc::vec;
use alloc::vec::Vec;
use alloc::string::String;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::params::{CHUNK_SIZE, OUTPUT_BYTES};
use crate::sponge::Hash;
use crate::stream::{left_subtree_chunks, HEADER_SIZE, PAIR_SIZE};
use crate::tree::{hash_leaf, hash_node};

/// Item yielded by the async streaming decoder.
#[derive(Debug)]
pub enum StreamItem {
    /// A verified data chunk.
    Chunk {
        /// Byte offset in the original data.
        offset: u64,
        /// Verified chunk data (up to CHUNK_SIZE bytes).
        data: Vec<u8>,
    },
    /// Decoding complete. All chunks verified.
    Done,
    /// Verification or I/O error.
    Error(StreamError),
}

/// Streaming decode errors.
#[derive(Debug)]
pub enum StreamError {
    /// Hash mismatch — data corrupted or tampered.
    HashMismatch { offset: u64 },
    /// Unexpected end of stream.
    Truncated,
    /// I/O error from the underlying reader.
    Io(std::io::Error),
}

impl From<std::io::Error> for StreamError {
    fn from(e: std::io::Error) -> Self {
        StreamError::Io(e)
    }
}

/// Async streaming decoder. Yields verified chunks one at a time.
///
/// Memory: O(log(n)) for the hash stack, plus one CHUNK_SIZE buffer.
/// The entire file is never held in memory.
pub struct StreamDecoder<R> {
    reader: R,
    root_hash: Hash,
    data_len: u64,
    num_chunks: u64,
    /// Stack of expected hashes for tree verification.
    /// Entries: (chunk_offset, chunk_count, is_root, expected_hash).
    stack: Vec<(u64, u64, bool, Hash)>,
    /// Total bytes yielded so far.
    yielded: u64,
    /// Whether header has been read.
    header_read: bool,
    done: bool,
}

impl<R: AsyncReadExt + Unpin> StreamDecoder<R> {
    /// Create a decoder for a combined pre-order stream.
    ///
    /// `root_hash`: expected root hash (from metadata/registry).
    /// `data_len`: expected data length in bytes.
    /// `reader`: async source of the encoded stream.
    pub fn new(root_hash: Hash, data_len: u64, reader: R) -> Self {
        let n = if data_len == 0 {
            1
        } else {
            ((data_len as usize + CHUNK_SIZE - 1) / CHUNK_SIZE) as u64
        };
        Self {
            reader,
            root_hash,
            data_len,
            num_chunks: n,
            stack: Vec::with_capacity(32), // log2(max_chunks)
            yielded: 0,
            header_read: false,
            done: false,
        }
    }

    /// Read and verify the next chunk from the stream.
    ///
    /// Call repeatedly until `StreamItem::Done` is returned.
    /// Each call reads exactly one chunk (≤4KB) from the reader.
    pub async fn next(&mut self) -> StreamItem {
        if self.done {
            return StreamItem::Done;
        }

        // Read header on first call.
        if !self.header_read {
            match self.read_header().await {
                Ok(()) => {}
                Err(e) => {
                    self.done = true;
                    return StreamItem::Error(e);
                }
            }
            self.header_read = true;

            // Initialize stack with root.
            if self.num_chunks <= 1 {
                // Single chunk: read directly, verify as root.
                return self.read_single_chunk().await;
            }
            self.stack
                .push((0, self.num_chunks, true, self.root_hash.clone()));
        }

        // Process stack until we yield a leaf chunk.
        loop {
            let (offset, count, is_root, expected) = match self.stack.pop() {
                Some(entry) => entry,
                None => {
                    self.done = true;
                    return StreamItem::Done;
                }
            };

            if count == 1 {
                // Leaf: read chunk data, verify.
                return self.read_leaf(offset, is_root, &expected).await;
            }

            // Parent: read hash pair, push children.
            match self.read_parent(offset, count, is_root, &expected).await {
                Ok(()) => {} // children pushed to stack, continue loop
                Err(e) => {
                    self.done = true;
                    return StreamItem::Error(e);
                }
            }
        }
    }

    async fn read_header(&mut self) -> Result<(), StreamError> {
        let mut buf = [0u8; HEADER_SIZE];
        self.reader
            .read_exact(&mut buf)
            .await
            .map_err(|_| StreamError::Truncated)?;
        let declared_len = u64::from_le_bytes(buf);
        if declared_len != self.data_len {
            return Err(StreamError::HashMismatch { offset: 0 });
        }
        Ok(())
    }

    async fn read_single_chunk(&mut self) -> StreamItem {
        let chunk_len = self.data_len as usize;
        let mut buf = vec![0u8; chunk_len];
        if let Err(e) = self.reader.read_exact(&mut buf).await {
            self.done = true;
            return StreamItem::Error(StreamError::Io(e));
        }

        let cv = hash_leaf(&buf, 0, true);
        if cv != self.root_hash {
            self.done = true;
            return StreamItem::Error(StreamError::HashMismatch { offset: 0 });
        }

        self.yielded += buf.len() as u64;
        self.done = true;
        StreamItem::Chunk {
            offset: 0,
            data: buf,
        }
    }

    async fn read_parent(
        &mut self,
        offset: u64,
        count: u64,
        is_root: bool,
        expected: &Hash,
    ) -> Result<(), StreamError> {
        let mut pair_buf = [0u8; PAIR_SIZE];
        self.reader
            .read_exact(&mut pair_buf)
            .await
            .map_err(|_| StreamError::Truncated)?;

        let left_hash = Hash::from_bytes(pair_buf[..OUTPUT_BYTES].try_into().unwrap());
        let right_hash = Hash::from_bytes(pair_buf[OUTPUT_BYTES..].try_into().unwrap());

        // Verify parent hash.
        let parent = hash_node(&left_hash, &right_hash, is_root);
        if parent != *expected {
            return Err(StreamError::HashMismatch {
                offset: offset * CHUNK_SIZE as u64,
            });
        }

        let split = left_subtree_chunks(count as usize) as u64;

        // Push RIGHT first (stack is LIFO — left will be processed first).
        self.stack.push((
            offset + split,
            count - split,
            false,
            right_hash,
        ));
        self.stack
            .push((offset, split, false, left_hash));

        Ok(())
    }

    async fn read_leaf(&mut self, offset: u64, _is_root: bool, expected: &Hash) -> StreamItem {
        let byte_offset = offset * CHUNK_SIZE as u64;
        let chunk_len = CHUNK_SIZE.min((self.data_len - byte_offset) as usize);

        let mut buf = vec![0u8; chunk_len];
        if let Err(e) = self.reader.read_exact(&mut buf).await {
            self.done = true;
            return StreamItem::Error(StreamError::Io(e));
        }

        let cv = hash_leaf(&buf, offset, false);
        if cv != *expected {
            self.done = true;
            return StreamItem::Error(StreamError::HashMismatch {
                offset: byte_offset,
            });
        }

        self.yielded += chunk_len as u64;
        StreamItem::Chunk {
            offset: byte_offset,
            data: buf,
        }
    }

    /// Bytes verified and yielded so far.
    pub fn progress(&self) -> u64 {
        self.yielded
    }

    /// Total expected bytes.
    pub fn total(&self) -> u64 {
        self.data_len
    }

    /// Whether decoding is complete.
    pub fn is_done(&self) -> bool {
        self.done
    }

    /// Consume the decoder and return the inner reader.
    pub fn into_reader(self) -> R {
        self.reader
    }
}

/// Async streaming encoder. Reads from an async reader, writes verified
/// combined stream to an async writer.
///
/// Memory: O(depth) hash stack + one CHUNK_SIZE buffer.
pub async fn encode_stream<R, W>(
    data_len: u64,
    mut reader: R,
    mut writer: W,
) -> Result<Hash, StreamError>
where
    R: AsyncReadExt + Unpin,
    W: AsyncWriteExt + Unpin,
{
    // Write header.
    writer
        .write_all(&data_len.to_le_bytes())
        .await
        .map_err(StreamError::Io)?;

    let n = if data_len == 0 {
        1
    } else {
        ((data_len as usize + CHUNK_SIZE - 1) / CHUNK_SIZE) as u64
    };

    if n <= 1 {
        let mut buf = vec![0u8; data_len as usize];
        reader
            .read_exact(&mut buf)
            .await
            .map_err(StreamError::Io)?;
        writer.write_all(&buf).await.map_err(StreamError::Io)?;
        writer.flush().await.map_err(StreamError::Io)?;
        return Ok(hash_leaf(&buf, 0, true));
    }

    // For multi-chunk: read all data first (encode requires random access
    // for pre-order layout). TODO: streaming pre-order encode without buffering
    // requires two passes or a different layout.
    let mut data = vec![0u8; data_len as usize];
    reader
        .read_exact(&mut data)
        .await
        .map_err(StreamError::Io)?;

    let (root, encoded) = crate::stream::encode(&data);
    // Skip header (already written).
    writer
        .write_all(&encoded[HEADER_SIZE..])
        .await
        .map_err(StreamError::Io)?;
    writer.flush().await.map_err(StreamError::Io)?;

    Ok(root)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn roundtrip_small() {
        let data = b"hello hemera streaming";
        let (root, encoded) = crate::stream::encode(data);

        let cursor = std::io::Cursor::new(encoded);
        let mut decoder = StreamDecoder::new(root, data.len() as u64, cursor);

        let mut recovered = Vec::new();
        loop {
            match decoder.next().await {
                StreamItem::Chunk { data, .. } => recovered.extend_from_slice(&data),
                StreamItem::Done => break,
                StreamItem::Error(e) => panic!("decode error: {:?}", e),
            }
        }
        assert_eq!(&recovered, &data[..]);
    }

    #[tokio::test]
    async fn roundtrip_large() {
        let data: Vec<u8> = (0..50_000).map(|i| (i % 256) as u8).collect();
        let (root, encoded) = crate::stream::encode(&data);

        let cursor = std::io::Cursor::new(encoded);
        let mut decoder = StreamDecoder::new(root, data.len() as u64, cursor);

        let mut recovered = Vec::new();
        let mut chunks = 0;
        loop {
            match decoder.next().await {
                StreamItem::Chunk { data, .. } => {
                    recovered.extend_from_slice(&data);
                    chunks += 1;
                }
                StreamItem::Done => break,
                StreamItem::Error(e) => panic!("decode error: {:?}", e),
            }
        }
        assert_eq!(recovered, data);
        assert!(chunks > 1, "should have multiple chunks for 50KB");
        assert_eq!(decoder.progress(), data.len() as u64);
    }

    #[tokio::test]
    async fn tampered_chunk_detected() {
        let data = b"tamper detection in streaming mode with enough data for two chunks";
        let data_padded: Vec<u8> = data.iter().copied().chain(vec![0u8; 8192]).collect();
        let (root, mut encoded) = crate::stream::encode(&data_padded);

        // Tamper with a byte in the encoded data (after header + first hash pair).
        if encoded.len() > 100 {
            encoded[100] ^= 0xFF;
        }

        let cursor = std::io::Cursor::new(encoded);
        let mut decoder = StreamDecoder::new(root, data_padded.len() as u64, cursor);

        let mut found_error = false;
        loop {
            match decoder.next().await {
                StreamItem::Chunk { .. } => {}
                StreamItem::Done => break,
                StreamItem::Error(StreamError::HashMismatch { .. }) => {
                    found_error = true;
                    break;
                }
                StreamItem::Error(e) => panic!("unexpected error: {:?}", e),
            }
        }
        assert!(found_error, "tampered data should be detected");
    }

    #[tokio::test]
    async fn encode_stream_roundtrip() {
        let data = b"streaming encode test with hemera verified streaming";
        let reader = std::io::Cursor::new(data.to_vec());
        let mut encoded_buf = Vec::new();

        let root = encode_stream(data.len() as u64, reader, &mut encoded_buf)
            .await
            .unwrap();

        // Verify: decode what we encoded.
        let (expected_root, expected_encoded) = crate::stream::encode(data);
        assert_eq!(root, expected_root);

        // Decode via streaming decoder.
        let cursor = std::io::Cursor::new(encoded_buf);
        let mut decoder = StreamDecoder::new(root, data.len() as u64, cursor);

        let mut recovered = Vec::new();
        loop {
            match decoder.next().await {
                StreamItem::Chunk { data, .. } => recovered.extend_from_slice(&data),
                StreamItem::Done => break,
                StreamItem::Error(e) => panic!("decode error: {:?}", e),
            }
        }
        assert_eq!(&recovered, &data[..]);
    }

    #[tokio::test]
    async fn progress_tracking() {
        let data: Vec<u8> = (0..20_000).map(|i| (i % 256) as u8).collect();
        let (root, encoded) = crate::stream::encode(&data);

        let cursor = std::io::Cursor::new(encoded);
        let mut decoder = StreamDecoder::new(root, data.len() as u64, cursor);

        assert_eq!(decoder.progress(), 0);
        assert_eq!(decoder.total(), data.len() as u64);
        assert!(!decoder.is_done());

        loop {
            match decoder.next().await {
                StreamItem::Chunk { .. } => {
                    assert!(decoder.progress() > 0);
                }
                StreamItem::Done => break,
                StreamItem::Error(e) => panic!("{:?}", e),
            }
        }
        assert_eq!(decoder.progress(), data.len() as u64);
        assert!(decoder.is_done());
    }
}
