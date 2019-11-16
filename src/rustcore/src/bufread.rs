use bytes::BufMut;
use tokio::io::AsyncRead;

use std::future::Future;
use std::io::Result;
use std::pin::Pin;
use std::task::{Context, Poll};

pub fn read_buf<'a, A: AsyncRead, B: BufMut>(
    read: &'a mut A,
    buf: &'a mut B,
) -> impl Future<Output = Result<usize>> + 'a {
    ReadBuf::new(read, buf)
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

impl<'a, A: AsyncRead, B: BufMut> Future for ReadBuf<'a, A, B> {
    type Output = Result<usize>;

    fn poll(self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Self::Output> {
        let (read_pin, buf) = unsafe {
            let me = Pin::into_inner_unchecked(self);
            (Pin::new_unchecked(&mut *me.read), &mut *me.buf)
        };

        read_pin.poll_read_buf(ctx, buf)
    }
}
