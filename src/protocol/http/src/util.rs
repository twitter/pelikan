use std::io::Write;

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
        let data = self.chunk_mut();
        let slice = std::slice::from_raw_parts(data.as_mut_ptr(), cnt);
        let _ = std::io::stdout().write_all(slice);

        self.count += cnt;
        self.buf.advance_mut(cnt)
    }

    fn chunk_mut(&mut self) -> &mut UninitSlice {
        self.buf.chunk_mut()
    }
}
