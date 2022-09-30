// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use bytes::buf::UninitSlice;
use protocol_common::BufMut;

pub(crate) struct CountingBuf<B> {
    buf: B,
    count: usize,
}

impl<B> CountingBuf<B> {
    pub fn new(buf: B) -> Self {
        Self { buf, count: 0 }
    }

    pub fn count(&self) -> usize {
        self.count
    }
}

unsafe impl<B> BufMut for CountingBuf<B>
where
    B: BufMut,
{
    fn remaining_mut(&self) -> usize {
        self.buf.remaining_mut()
    }

    unsafe fn advance_mut(&mut self, cnt: usize) {
        self.count += cnt;
        self.buf.advance_mut(cnt)
    }

    fn chunk_mut(&mut self) -> &mut UninitSlice {
        self.buf.chunk_mut()
    }
}
