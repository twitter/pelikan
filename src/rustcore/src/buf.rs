use bytes::{Buf, BufMut};
use tokio::io::{AsyncRead, AsyncWrite};

use std::future::Future;
use std::io::Result;
use std::pin::Pin;
use std::task::{Context, Poll};

/// Create a future for `poll_read_buf` in `AsyncRead`.
pub fn read_buf<'a, A: AsyncRead, B: BufMut + Unpin>(
    read: &'a mut A,
    buf: &'a mut B,
) -> impl Future<Output = Result<usize>> + 'a {
    ReadBuf::new(read, buf)
}

/// Create a future for `poll_write_buf` in `AsyncWrite`.
pub fn write_buf<'a, A: AsyncWrite, B: Buf + Unpin>(
    write: &'a mut A,
    buf: &'a mut B,
) -> impl Future<Output = Result<usize>> + 'a {
    WriteBuf::new(write, buf)
}

pub struct ReadBuf<'a, A: AsyncRead, B: BufMut> {
    read: &'a mut A,
    buf: &'a mut B,
}

impl<'a, A: AsyncRead, B: BufMut> ReadBuf<'a, A, B> {
    pub fn new(read: &'a mut A, buf: &'a mut B) -> Self {
        Self { read, buf }
    }
}

impl<'a, A: AsyncRead, B: BufMut + Unpin> Future for ReadBuf<'a, A, B> {
    type Output = Result<usize>;

    fn poll(self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Self::Output> {
        // Safe since we are only performing pin projection and
        // self.buf is Unpin
        let (read_pin, buf) = unsafe {
            let me = Pin::into_inner_unchecked(self);
            (Pin::new_unchecked(&mut *me.read), &mut *me.buf)
        };

        read_pin.poll_read_buf(ctx, buf)
    }
}

pub struct WriteBuf<'a, A: AsyncWrite, B: Buf> {
    write: &'a mut A,
    buf: &'a mut B,
}

impl<'a, A: AsyncWrite, B: Buf> WriteBuf<'a, A, B> {
    pub fn new(write: &'a mut A, buf: &'a mut B) -> Self {
        Self { write, buf }
    }
}

impl<'a, A: AsyncWrite, B: Buf + Unpin> Future for WriteBuf<'a, A, B> {
    type Output = Result<usize>;

    fn poll(self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Self::Output> {
        // Safe since we are only performing pin projection
        // and self.buf is unpin.
        let (write_pin, buf) = unsafe {
            let me = Pin::into_inner_unchecked(self);
            (Pin::new_unchecked(&mut *me.write), &mut *me.buf)
        };

        write_pin.poll_write_buf(ctx, buf)
    }
}
