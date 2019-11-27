// Copyright (C) 2019 Twitter, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::future::Future;
use std::io::Result as IOResult;
use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::{Buf, BufMut};
use ccommon::buf::OwnedBuf;
use tokio::io::{AsyncRead, AsyncWrite};

use crate::WorkerMetrics;

#[derive(Debug, Error)]
pub enum BufIOError {
    #[error("Out of memory")]
    OutOfMemory,
    #[error("Stream is closed")]
    StreamClosed,
    #[error("IO Error")]
    IOError(#[from] std::io::Error),
}

pub async fn read_buf<'a, A: AsyncRead + Unpin>(
    stream: &'a mut A,
    buf: &'a mut OwnedBuf,
    metrics: &'a WorkerMetrics,
) -> Result<usize, BufIOError> {
    buf.lshift();

    loop {
        break match ReadBuf::new(stream, buf).await {
            Ok(0) => {
                if buf.write_size() == 0 {
                    metrics.socket_read.incr();

                    match buf.double() {
                        Ok(()) => continue,
                        Err(_) => Err(BufIOError::OutOfMemory),
                    }
                } else {
                    // This can occur when a the other end of the connection
                    // disappears. At this point we can just close the connection
                    // as otherwise we will continuously read 0 and waste CPU
                    Err(BufIOError::StreamClosed)
                }
            }
            Ok(nbytes) => {
                metrics.bytes_read.incr_n(nbytes as u64);
                metrics.socket_read.incr();
                Ok(nbytes)
            }
            Err(e) => {
                metrics.socket_read_ex.incr();
                Err(e.into())
            }
        };
    }
}

pub async fn write_buf<'a, A: AsyncWrite + Unpin>(
    stream: &'a mut A,
    buf: &'a mut OwnedBuf,
    metrics: &'a WorkerMetrics,
) -> Result<usize, BufIOError> {
    let bufsize = buf.read_size();

    while buf.read_size() > 0 {
        let nbytes = match WriteBuf::new(stream, buf).await {
            Ok(nbytes) => nbytes,
            Err(e) => {
                metrics.socket_write_ex.incr();
                return Err(e.into());
            }
        };

        metrics.socket_write.incr();
        metrics.bytes_sent.incr_n(nbytes as u64);
    }

    buf.lshift();

    Ok(bufsize)
}

pub(crate) struct ReadBuf<'a, A: AsyncRead + Unpin, B: BufMut> {
    read: &'a mut A,
    buf: &'a mut B,
}

impl<'a, A: AsyncRead + Unpin, B: BufMut> ReadBuf<'a, A, B> {
    pub fn new(read: &'a mut A, buf: &'a mut B) -> Self {
        Self { read, buf }
    }
}

impl<'a, A: AsyncRead + Unpin, B: BufMut + Unpin> Future for ReadBuf<'a, A, B> {
    type Output = IOResult<usize>;

    fn poll(mut self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Self::Output> {
        let Self { read, buf } = &mut *self;
        Pin::new(read).poll_read_buf(ctx, buf)
    }
}

pub(crate) struct WriteBuf<'a, A: AsyncWrite + Unpin, B: Buf> {
    write: &'a mut A,
    buf: &'a mut B,
}

impl<'a, A: AsyncWrite + Unpin, B: Buf> WriteBuf<'a, A, B> {
    pub fn new(write: &'a mut A, buf: &'a mut B) -> Self {
        Self { write, buf }
    }
}

impl<'a, A: AsyncWrite + Unpin, B: Buf + Unpin> Future for WriteBuf<'a, A, B> {
    type Output = IOResult<usize>;

    fn poll(mut self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Self::Output> {
        let Self { write, buf } = &mut *self;
        Pin::new(write).poll_write_buf(ctx, buf)
    }
}
