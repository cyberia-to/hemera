// ---
// tags: hemera, rust
// crystal-type: source
// crystal-domain: comp
// ---
//! Minimal async I/O traits for hemera verified streaming.
//!
//! Provides [`AsyncRead`] and [`AsyncWrite`] traits with `read_exact`,
//! `write_all`, and `flush` helpers. Zero external dependencies — the
//! traits use only `core::task` and `std::io` types.
//!
//! Tokio, async-std, smol, and embassy users implement these two traits
//! for their reader/writer types (a 10-line adapter each).

extern crate std;

use core::pin::Pin;
use core::task::{Context, Poll};

/// Async byte reader.
pub trait AsyncRead {
    /// Attempt to read bytes into `buf`, returning how many were read.
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<std::io::Result<usize>>;
}

/// Async byte writer.
pub trait AsyncWrite {
    /// Attempt to write bytes from `buf`, returning how many were written.
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>>;

    /// Flush buffered output.
    fn poll_flush(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<std::io::Result<()>>;
}

// ── Helpers ─────────────────────────────────────────────────────

/// Read exactly `buf.len()` bytes, or return an error.
pub async fn read_exact<R: AsyncRead + Unpin>(
    reader: &mut R,
    buf: &mut [u8],
) -> std::io::Result<()> {
    let mut filled = 0;
    while filled < buf.len() {
        let n = core::future::poll_fn(|cx| {
            Pin::new(&mut *reader).poll_read(cx, &mut buf[filled..])
        })
        .await?;
        if n == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "unexpected end of stream",
            ));
        }
        filled += n;
    }
    Ok(())
}

/// Write all bytes in `buf`.
pub async fn write_all<W: AsyncWrite + Unpin>(
    writer: &mut W,
    buf: &[u8],
) -> std::io::Result<()> {
    let mut written = 0;
    while written < buf.len() {
        let n = core::future::poll_fn(|cx| {
            Pin::new(&mut *writer).poll_write(cx, &buf[written..])
        })
        .await?;
        if n == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::WriteZero,
                "write returned 0",
            ));
        }
        written += n;
    }
    Ok(())
}

/// Flush a writer.
pub async fn flush<W: AsyncWrite + Unpin>(writer: &mut W) -> std::io::Result<()> {
    core::future::poll_fn(|cx| Pin::new(&mut *writer).poll_flush(cx)).await
}

// ── Blanket impls for std types ────────────────────────────────

impl<T: AsRef<[u8]> + Unpin> AsyncRead for std::io::Cursor<T> {
    fn poll_read(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<std::io::Result<usize>> {
        Poll::Ready(std::io::Read::read(self.get_mut(), buf))
    }
}

impl<W: AsyncWrite + Unpin + ?Sized> AsyncWrite for &mut W {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut **self.get_mut()).poll_write(cx, buf)
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut **self.get_mut()).poll_flush(cx)
    }
}

impl AsyncWrite for alloc::vec::Vec<u8> {
    fn poll_write(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Poll::Ready(std::io::Write::write(self.get_mut(), buf))
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<std::io::Result<()>> {
        Poll::Ready(std::io::Write::flush(self.get_mut()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A reader that yields `chunk_len` bytes per `poll_read` call, then
    /// returns `Ok(0)` (EOF) once `data` is exhausted.
    struct ChunkedReader {
        data: alloc::vec::Vec<u8>,
        pos: usize,
        chunk_len: usize,
    }

    impl AsyncRead for ChunkedReader {
        fn poll_read(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            buf: &mut [u8],
        ) -> Poll<std::io::Result<usize>> {
            let this = self.get_mut();
            let remaining = &this.data[this.pos..];
            let n = remaining.len().min(buf.len()).min(this.chunk_len);
            buf[..n].copy_from_slice(&remaining[..n]);
            this.pos += n;
            Poll::Ready(Ok(n))
        }
    }

    /// A reader that always reports zero bytes read (immediate EOF).
    struct EofReader;

    impl AsyncRead for EofReader {
        fn poll_read(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            _buf: &mut [u8],
        ) -> Poll<std::io::Result<usize>> {
            Poll::Ready(Ok(0))
        }
    }

    /// A writer that accepts `chunk_len` bytes per `poll_write` call.
    struct ChunkedWriter {
        written: alloc::vec::Vec<u8>,
        chunk_len: usize,
    }

    impl AsyncWrite for ChunkedWriter {
        fn poll_write(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<std::io::Result<usize>> {
            let this = self.get_mut();
            let n = buf.len().min(this.chunk_len);
            this.written.extend_from_slice(&buf[..n]);
            Poll::Ready(Ok(n))
        }
        fn poll_flush(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
        ) -> Poll<std::io::Result<()>> {
            Poll::Ready(Ok(()))
        }
    }

    /// A writer that always reports zero bytes written.
    struct ZeroWriter;

    impl AsyncWrite for ZeroWriter {
        fn poll_write(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            _buf: &[u8],
        ) -> Poll<std::io::Result<usize>> {
            Poll::Ready(Ok(0))
        }
        fn poll_flush(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
        ) -> Poll<std::io::Result<()>> {
            Poll::Ready(Ok(()))
        }
    }

    #[tokio::test]
    async fn read_exact_over_a_cursor_fills_the_buffer() {
        let mut reader = std::io::Cursor::new(alloc::vec![1u8, 2, 3, 4, 5]);
        let mut buf = [0u8; 5];
        read_exact(&mut reader, &mut buf).await.unwrap();
        assert_eq!(buf, [1, 2, 3, 4, 5]);
    }

    #[tokio::test]
    async fn read_exact_loops_across_short_reads() {
        let mut reader = ChunkedReader {
            data: alloc::vec![1u8, 2, 3, 4, 5, 6, 7],
            pos: 0,
            chunk_len: 2,
        };
        let mut buf = [0u8; 7];
        read_exact(&mut reader, &mut buf).await.unwrap();
        assert_eq!(buf, [1, 2, 3, 4, 5, 6, 7]);
    }

    #[tokio::test]
    async fn read_exact_reports_unexpected_eof_on_a_short_source() {
        let mut reader = std::io::Cursor::new(alloc::vec![1u8, 2]);
        let mut buf = [0u8; 5];
        let err = read_exact(&mut reader, &mut buf).await.unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::UnexpectedEof);
    }

    #[tokio::test]
    async fn read_exact_on_an_immediately_exhausted_reader_errors_without_looping_forever() {
        let mut reader = EofReader;
        let mut buf = [0u8; 1];
        let err = read_exact(&mut reader, &mut buf).await.unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::UnexpectedEof);
    }

    #[tokio::test]
    async fn read_exact_on_an_empty_buffer_succeeds_without_reading() {
        let mut reader = EofReader;
        let mut buf: [u8; 0] = [];
        read_exact(&mut reader, &mut buf).await.unwrap();
    }

    #[tokio::test]
    async fn write_all_over_a_vec_appends_every_byte() {
        let mut writer: alloc::vec::Vec<u8> = alloc::vec::Vec::new();
        write_all(&mut writer, &[1, 2, 3]).await.unwrap();
        assert_eq!(writer, alloc::vec![1u8, 2, 3]);
    }

    #[tokio::test]
    async fn write_all_loops_across_short_writes() {
        let mut writer = ChunkedWriter {
            written: alloc::vec::Vec::new(),
            chunk_len: 2,
        };
        write_all(&mut writer, &[1, 2, 3, 4, 5]).await.unwrap();
        assert_eq!(writer.written, alloc::vec![1u8, 2, 3, 4, 5]);
    }

    #[tokio::test]
    async fn write_all_reports_write_zero_on_a_stuck_sink() {
        let mut writer = ZeroWriter;
        let err = write_all(&mut writer, &[1, 2, 3]).await.unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::WriteZero);
    }

    #[tokio::test]
    async fn write_all_on_an_empty_buffer_succeeds_without_writing() {
        let mut writer = ZeroWriter;
        write_all(&mut writer, &[]).await.unwrap();
    }

    #[tokio::test]
    async fn flush_delegates_to_the_writer() {
        let mut writer: alloc::vec::Vec<u8> = alloc::vec::Vec::new();
        flush(&mut writer).await.unwrap();
    }
}
