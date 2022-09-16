// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

pub use bytes::buf::UninitSlice;
pub use bytes::{Buf, BufMut};

use crate::*;
use core::borrow::{Borrow, BorrowMut};
use std::alloc::*;

const KB: usize = 1024;
const MB: usize = 1024 * KB;

/// A simple growable byte buffer, represented as a contiguous range of bytes
pub struct Buffer {
    ptr: *mut u8,
    cap: usize,
    read_offset: usize,
    write_offset: usize,
    target_size: usize,
}

unsafe impl Send for Buffer {}
unsafe impl Sync for Buffer {}

impl Buffer {
    /// Create a new buffer that can hold up to `target_size` bytes without
    /// resizing. The buffer may grow beyond the `target_size`, but will shrink
    /// back down to the `target_size` when possible.
    pub fn new(target_size: usize) -> Self {
        let target_size = target_size.next_power_of_two();
        let layout = Layout::array::<u8>(target_size).unwrap();
        let ptr = unsafe { alloc(layout) };
        let cap = target_size;
        let read_offset = 0;
        let write_offset = 0;

        SESSION_BUFFER_BYTE.add(cap as _);

        Self {
            ptr,
            cap,
            read_offset,
            write_offset,
            target_size,
        }
    }

    /// Returns the current capacity of the buffer.
    pub fn capacity(&self) -> usize {
        self.cap
    }

    /// Reserve space for `amt` additional bytes.
    pub fn reserve(&mut self, amt: usize) {
        // if the buffer is empty, reset the offsets
        if self.remaining() == 0 {
            self.read_offset = 0;
            self.write_offset = 0;
        }

        // grow the buffer if needed, uses a multiple of the target size
        if amt > self.remaining_mut() {
            // calculate the required buffer size
            let size = self.write_offset + amt;

            // determine what power of the target size would be required to
            // hold the new size
            let pow = (size).next_power_of_two();

            // determine how much to grow the buffer by
            let amt = if size > MB || pow > MB {
                // if it would be above a MB, determine the next whole MB and
                // subtract the current capacity to determine the amount to grow
                // by
                (size / MB + 1) * MB - self.cap
            } else {
                // if it would be 1 MB or less, set it to the next power of two
                // multiple of the target size, minus the current capacity
                pow - self.cap
            };

            SESSION_BUFFER_BYTE.add(amt as _);

            // new size will be the current capacity plus the amount needed
            let size = self.cap + amt;
            let layout = Layout::array::<u8>(self.cap).unwrap();
            self.ptr = unsafe { realloc(self.ptr, layout, size) };
            self.cap = size;
        }
    }

    /// Clear the buffer.
    pub fn clear(&mut self) {
        self.read_offset = 0;
        self.write_offset = 0;

        // if the buffer is oversized, shrink to the target size
        if self.cap > self.target_size {
            trace!("shrinking buffer");

            SESSION_BUFFER_BYTE.sub((self.cap - self.target_size) as _);

            let layout = Layout::array::<u8>(self.cap).unwrap();
            self.ptr = unsafe { realloc(self.ptr, layout, self.target_size) };
            self.cap = self.target_size;
        }
    }

    /// Compact the buffer by moving contents to the beginning and freeing any
    /// excess space. As an optimization, this will not always compact the
    /// buffer to its `target_size`.
    pub fn compact(&mut self) {
        // if the buffer is empty, we clear the buffer and return
        if self.remaining() == 0 {
            self.clear();
            return;
        }

        // if its not too large, we don't compact
        if self.cap == self.target_size {
            return;
        }

        // if the buffer data is deep into the buffer, we can copy the data to
        // the start of the buffer to make additional space available for writes
        if self.read_offset > self.target_size {
            if self.remaining() < self.read_offset {
                unsafe {
                    std::ptr::copy_nonoverlapping(
                        self.ptr.add(self.read_offset),
                        self.ptr,
                        self.remaining(),
                    );
                }
            } else {
                unsafe {
                    std::ptr::copy(self.ptr.add(self.read_offset), self.ptr, self.remaining());
                }
            }
            self.write_offset = self.remaining();
            self.read_offset = 0;
        }

        let target = if self.write_offset > MB {
            (1 + (self.write_offset / MB)) * MB
        } else {
            self.write_offset.next_power_of_two()
        };

        SESSION_BUFFER_BYTE.sub((self.cap - target) as _);
        let layout = Layout::array::<u8>(self.cap).unwrap();
        self.ptr = unsafe { realloc(self.ptr, layout, target) };
        self.cap = target;
    }

    /// Get the current write position as a pointer. `remaining_mut` should be
    /// used as the length.
    pub fn write_ptr(&mut self) -> *mut u8 {
        unsafe { self.ptr.add(self.write_offset) }
    }

    /// Get the current read position as a pointer. `remaining` should be used
    /// as the length.
    pub fn read_ptr(&mut self) -> *mut u8 {
        unsafe { self.ptr.add(self.read_offset) }
    }
}

impl Drop for Buffer {
    fn drop(&mut self) {
        SESSION_BUFFER_BYTE.sub(self.cap as _);
    }
}

impl Borrow<[u8]> for Buffer {
    fn borrow(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.ptr.add(self.read_offset), self.remaining()) }
    }
}

impl BorrowMut<[u8]> for Buffer {
    fn borrow_mut(self: &mut Buffer) -> &mut [u8] {
        unsafe {
            std::slice::from_raw_parts_mut(self.ptr.add(self.write_offset), self.remaining_mut())
        }
    }
}

impl Buf for Buffer {
    fn remaining(&self) -> usize {
        self.write_offset - self.read_offset
    }

    fn chunk(&self) -> &[u8] {
        self.borrow()
    }

    fn advance(&mut self, amt: usize) {
        self.read_offset = std::cmp::min(self.read_offset + amt, self.write_offset);
        self.compact();
    }
}

unsafe impl BufMut for Buffer {
    fn remaining_mut(&self) -> usize {
        self.cap - self.write_offset
    }

    unsafe fn advance_mut(&mut self, amt: usize) {
        self.write_offset = std::cmp::min(self.write_offset + amt, self.cap);
    }

    fn chunk_mut(&mut self) -> &mut bytes::buf::UninitSlice {
        unsafe {
            UninitSlice::from_raw_parts_mut(self.ptr.add(self.write_offset), self.remaining_mut())
        }
    }

    fn put<T: Buf>(&mut self, mut src: T)
    where
        Self: Sized,
    {
        while src.has_remaining() {
            let chunk = src.chunk();
            let len = chunk.len();
            self.put_slice(chunk);
            src.advance(len);
        }
    }

    fn put_slice(&mut self, src: &[u8]) {
        self.reserve(src.len());
        assert!(self.remaining_mut() >= src.len());
        unsafe {
            std::ptr::copy_nonoverlapping(src.as_ptr(), self.ptr.add(self.write_offset), src.len());
        }
        unsafe {
            self.advance_mut(src.len());
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::*;
    use std::borrow::Borrow;

    #[test]
    // test buffer initialization with various capacities
    fn new() {
        let buffer = Buffer::new(1024);
        assert_eq!(buffer.remaining(), 0);
        assert_eq!(buffer.remaining_mut(), 1024);

        let buffer = Buffer::new(2048);
        assert_eq!(buffer.remaining(), 0);
        assert_eq!(buffer.remaining_mut(), 2048);

        // test zero capacity buffer, rounds to 1 byte buffer
        let buffer = Buffer::new(0);
        assert_eq!(buffer.remaining(), 0);
        assert_eq!(buffer.remaining_mut(), 1);

        // test with non power of 2, rounds to next power of two
        let buffer = Buffer::new(100);
        assert_eq!(buffer.remaining(), 0);
        assert_eq!(buffer.remaining_mut(), 128);
    }

    #[test]
    // tests a small buffer growing only on second write
    fn write_1() {
        let mut buffer = Buffer::new(8);
        assert_eq!(buffer.remaining(), 0);
        assert_eq!(buffer.remaining_mut(), 8);

        // first write fits in buffer
        buffer.put_slice(b"GET ");
        assert_eq!(buffer.remaining(), 4);
        assert_eq!(buffer.remaining_mut(), 4);

        let content: &[u8] = buffer.borrow();
        assert_eq!(content, b"GET ");

        // second write causes buffer to grow
        buffer.put_slice(b"SOME_KEY\r\n");
        assert_eq!(buffer.remaining(), 14);
        assert_eq!(buffer.remaining_mut(), 2);

        let content: &[u8] = buffer.borrow();
        assert_eq!(content, b"GET SOME_KEY\r\n");
    }

    #[test]
    // test a zero capacity buffer growing on two consecutive writes
    fn write_2() {
        let mut buffer = Buffer::new(0);
        assert_eq!(buffer.remaining(), 0);
        assert_eq!(buffer.remaining_mut(), 1);

        // zero capacity buffer grows on first write
        buffer.put_slice(b"GET KEY\r\n");
        assert_eq!(buffer.remaining(), 9);
        assert_eq!(buffer.remaining_mut(), 7);

        let content: &[u8] = buffer.borrow();
        assert_eq!(content, b"GET KEY\r\n");

        // and again on second write
        buffer.put_slice(b"SET OTHER_KEY 0 0 1\r\nA\r\n");
        assert_eq!(buffer.remaining(), 33);
        assert_eq!(buffer.remaining_mut(), 31);

        let content: &[u8] = buffer.borrow();
        assert_eq!(content, b"GET KEY\r\nSET OTHER_KEY 0 0 1\r\nA\r\n");
    }

    #[test]
    // tests a large buffer that grows on first write
    fn write_3() {
        let mut buffer = Buffer::new(16);
        assert_eq!(buffer.remaining(), 0);
        assert_eq!(buffer.remaining_mut(), 16);

        buffer.put_slice(b"SET SOME_REALLY_LONG_KEY 0 0 1\r\nA\r\n");
        assert_eq!(buffer.remaining(), 35);
        assert_eq!(buffer.remaining_mut(), 29);
    }

    #[test]
    // tests a consume operation where all bytes are consumed and the buffer
    // remains its original size
    fn consume_1() {
        let mut buffer = Buffer::new(16);
        assert_eq!(buffer.remaining(), 0);
        assert_eq!(buffer.remaining_mut(), 16);

        buffer.put_slice(b"END\r\n");
        assert_eq!(buffer.remaining(), 5);
        assert_eq!(buffer.remaining_mut(), 11);

        buffer.advance(5);
        assert_eq!(buffer.remaining(), 0);
        assert_eq!(buffer.remaining_mut(), 16);
    }

    #[test]
    // tests a consume operation where all bytes are consumed and the buffer
    // shrinks to its original size
    fn consume_2() {
        let mut buffer = Buffer::new(2);
        assert_eq!(buffer.remaining(), 0);
        assert_eq!(buffer.remaining_mut(), 2);

        // buffer extends to the next power of two
        // with 5 byte message we need 8 bytes for the buffer
        buffer.put_slice(b"END\r\n");
        assert_eq!(buffer.remaining(), 5);
        assert_eq!(buffer.remaining_mut(), 3);

        buffer.advance(5);
        assert_eq!(buffer.remaining(), 0);
        assert_eq!(buffer.remaining_mut(), 2);
    }

    #[test]
    // tests a consume operation where not all bytes are consumed and buffer
    // remains its original size
    fn consume_3() {
        let mut buffer = Buffer::new(8);
        assert_eq!(buffer.remaining(), 0);
        assert_eq!(buffer.remaining_mut(), 8);

        let content = b"END\r\n";

        buffer.put_slice(content);
        assert_eq!(buffer.remaining(), 5);
        assert_eq!(buffer.remaining_mut(), 3);

        // consume all but the last byte of content in the buffer, one byte at
        // a time
        // - buffer len decreases with each call to consume()
        // - buffer available capacity stays the same
        for i in 1..5 {
            buffer.advance(1);
            assert_eq!(buffer.remaining(), 5 - i);
            assert_eq!(buffer.remaining_mut(), 3);
        }

        // when consuming the final byte, the read/write offsets move to the
        // start of the buffer, and available capacity should be the original
        // buffer size
        buffer.advance(1);
        assert_eq!(buffer.remaining(), 0);
        assert_eq!(buffer.remaining_mut(), 8);
    }

    #[test]
    // tests a consume operation where not all bytes are consumed and buffer
    // shrinks as bytes are consumed
    fn consume_4() {
        let mut buffer = Buffer::new(16);
        assert_eq!(buffer.remaining(), 0);
        assert_eq!(buffer.remaining_mut(), 16);

        let content = b"VALUE SOME_REALLY_LONG_KEY 0 1\r\n1\r\nEND\r\n";

        // buffer resizes up to 64 bytes to hold 40 bytes
        buffer.put_slice(content);
        assert_eq!(buffer.remaining(), 40);
        assert_eq!(buffer.remaining_mut(), 24);

        // partial consume, len decrease, no compact
        buffer.advance(8);
        assert_eq!(buffer.remaining(), 32);
        assert_eq!(buffer.remaining_mut(), 24);

        // consume one more byte, still no compact
        buffer.advance(1);
        assert_eq!(buffer.remaining(), 31);
        assert_eq!(buffer.remaining_mut(), 24);

        // partial consume, remaining drops to best fitting power of two
        buffer.advance(15);
        assert_eq!(buffer.remaining(), 16);
        assert_eq!(buffer.remaining_mut(), 0);

        // from here on, buffer will not shrink below target capacity and will
        // not compact

        // partial consume, since the buffer is the target size already, there
        // will be no compaction
        buffer.advance(1);
        assert_eq!(buffer.remaining(), 15);
        assert_eq!(buffer.remaining_mut(), 0);

        // consume all but the final byte
        // partial consume, len decrease
        // length = 1, size = 16, capacity = 15
        buffer.advance(14);
        assert_eq!(buffer.remaining(), 1);
        assert_eq!(buffer.remaining_mut(), 0);

        // on the final advance, all bytes are consumed and the entire buffer
        // is now clear

        // consume the final byte
        // length = 0, size = 16, capacity = 16
        buffer.advance(1);
        assert_eq!(buffer.remaining(), 0);
        assert_eq!(buffer.remaining_mut(), 16);
    }
}
