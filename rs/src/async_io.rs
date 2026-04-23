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
