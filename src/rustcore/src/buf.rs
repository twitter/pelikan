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

use bytes::{Buf, BufMut};
use tokio::io::{AsyncRead, AsyncWrite};

use std::future::Future;
use std::io::Result;
use std::pin::Pin;
use std::task::{Context, Poll};

/// Create a future for `poll_read_buf` in `AsyncRead`.
pub fn read_buf<'a, A: AsyncRead + Unpin, B: BufMut + Unpin>(
    read: &'a mut A,
    buf: &'a mut B,
) -> impl Future<Output = Result<usize>> + 'a {
    ReadBuf::new(read, buf)
}

/// Create a future for `poll_write_buf` in `AsyncWrite`.
pub fn write_buf<'a, A: AsyncWrite + Unpin, B: Buf + Unpin>(
    write: &'a mut A,
    buf: &'a mut B,
) -> impl Future<Output = Result<usize>> + 'a {
    WriteBuf::new(write, buf)
}

struct ReadBuf<'a, A: AsyncRead + Unpin, B: BufMut> {
    read: &'a mut A,
    buf: &'a mut B,
}

impl<'a, A: AsyncRead + Unpin, B: BufMut> ReadBuf<'a, A, B> {
    pub fn new(read: &'a mut A, buf: &'a mut B) -> Self {
        Self { read, buf }
    }
}

impl<'a, A: AsyncRead + Unpin, B: BufMut + Unpin> Future for ReadBuf<'a, A, B> {
    type Output = Result<usize>;

    fn poll(mut self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Self::Output> {
        let Self { read, buf } = &mut *self;
        Pin::new(read).poll_read_buf(ctx, buf)
    }
}

struct WriteBuf<'a, A: AsyncWrite + Unpin, B: Buf> {
    write: &'a mut A,
    buf: &'a mut B,
}

impl<'a, A: AsyncWrite + Unpin, B: Buf> WriteBuf<'a, A, B> {
    pub fn new(write: &'a mut A, buf: &'a mut B) -> Self {
        Self { write, buf }
    }
}

impl<'a, A: AsyncWrite + Unpin, B: Buf + Unpin> Future for WriteBuf<'a, A, B> {
    type Output = Result<usize>;

    fn poll(mut self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Self::Output> {
        let Self { write, buf } = &mut *self;
        Pin::new(write).poll_write_buf(ctx, buf)
    }
}
