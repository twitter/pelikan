// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! A very simple buffer type that can be replaced in the future.

use crate::SESSION_BUFFER_BYTE;

use core::borrow::{Borrow, BorrowMut};

/// A growable byte buffer
pub struct Buffer {
    buffer: Vec<u8>,
    read_offset: usize,
    write_offset: usize,
    target_capacity: usize,
}

impl Buffer {
    /// Create a new `Buffer` that can hold up to `capacity` bytes without
    /// re-allocating.
    #[allow(clippy::slow_vector_initialization)]
    pub fn with_capacity(capacity: usize) -> Self {
        let mut buffer = Vec::with_capacity(capacity);
        buffer.resize(capacity, 0);

        SESSION_BUFFER_BYTE.add(buffer.capacity() as _);

        Self {
            buffer,
            read_offset: 0,
            write_offset: 0,
            target_capacity: capacity,
        }
    }

    /// Returns the amount of space available to write into the buffer without
    /// reallocating.
    pub fn available_capacity(&self) -> usize {
        self.buffer.len() - self.write_offset
    }

    /// Return the number of bytes currently in the buffer.
    pub fn len(&self) -> usize {
        self.write_offset - self.read_offset
    }

    /// Check if the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    // TODO(bmartin): we're currently relying on the resize behaviors of the
    // underlying `Vec` storage. This currently results in growth to the next
    // nearest power of two. Effectively resulting in buffer doubling when a
    // resize is required.
    /// Reserve room for `additional` bytes in the buffer. This may reserve more
    /// space than requested to avoid frequent allocations. If the buffer
    /// already has sufficient available capacity, this is a no-op.
    pub fn reserve(&mut self, additional: usize) {
        let old_cap = self.buffer.capacity();
        let needed = additional.saturating_sub(self.available_capacity());
        if needed > 0 {
            let current = self.buffer.len();
            let target = (current + needed).next_power_of_two();
            self.buffer.resize(target, 0);
            SESSION_BUFFER_BYTE.add((self.buffer.capacity() - old_cap) as _);
        }
    }

    /// Append the bytes from `other` onto `self`.
    pub fn extend_from_slice(&mut self, other: &[u8]) {
        self.reserve(other.len());
        self.buffer[self.write_offset..(self.write_offset + other.len())].copy_from_slice(other);
        self.increase_len(other.len());
    }

    /// Mark that `amt` bytes have been consumed and should not be returned in
    /// future reads from the buffer.
    pub fn consume(&mut self, bytes: usize) {
        let old_capacity = self.buffer.capacity();
        self.read_offset = std::cmp::min(self.read_offset + bytes, self.write_offset);

        // if we have content, before shrinking we must shift content left
        if !self.is_empty() {
            self.buffer
                .copy_within(self.read_offset..self.write_offset, 0);
        }

        self.write_offset -= self.read_offset;
        self.read_offset = 0;

        // determine the target size of the buffer
        let target_size = if self.len() * 2 > self.buffer.len() {
            // buffer too full to shrink, early return
            return;
        } else if self.len() > self.target_capacity {
            // should shrink, but not to target capacity
            self.buffer.len() / 2
        } else {
            // shrink down to target capacity
            self.target_capacity
        };

        // buffer can be reduced to the target_size determined above
        self.buffer.truncate(target_size);
        self.buffer.shrink_to_fit();

        // update stats if the buffer has resized
        SESSION_BUFFER_BYTE.sub(old_capacity as i64 - self.buffer.capacity() as i64);
    }

    /// Marks the buffer as now containing `amt` additional bytes. This function
    /// prevents advancing the write offset beyond the initialized area of the
    /// underlying storage.
    pub fn increase_len(&mut self, amt: usize) {
        self.write_offset = std::cmp::min(self.write_offset + amt, self.buffer.len());
    }
}

impl Borrow<[u8]> for Buffer {
    fn borrow(&self) -> &[u8] {
        &self.buffer[self.read_offset..self.write_offset]
    }
}

impl BorrowMut<[u8]> for Buffer {
    fn borrow_mut(&mut self) -> &mut [u8] {
        let available = self.buffer.len();
        &mut self.buffer[self.write_offset..available]
    }
}

impl Drop for Buffer {
    fn drop(&mut self) {
        SESSION_BUFFER_BYTE.sub(self.buffer.capacity() as _);
    }
}

#[cfg(test)]
mod tests {
    use crate::Buffer;
    use std::borrow::Borrow;

    #[test]
    // test buffer initialization with various capacities
    fn new() {
        let buffer = Buffer::with_capacity(1024);
        assert_eq!(buffer.len(), 0);
        assert_eq!(buffer.available_capacity(), 1024);
        assert!(buffer.is_empty());

        let buffer = Buffer::with_capacity(2048);
        assert_eq!(buffer.len(), 0);
        assert_eq!(buffer.available_capacity(), 2048);
        assert!(buffer.is_empty());

        // test zero capacity buffer
        let buffer = Buffer::with_capacity(0);
        assert_eq!(buffer.len(), 0);
        assert_eq!(buffer.available_capacity(), 0);
        assert!(buffer.is_empty());

        // test with non power of 2
        let buffer = Buffer::with_capacity(100);
        assert_eq!(buffer.len(), 0);
        assert_eq!(buffer.available_capacity(), 100);
        assert!(buffer.is_empty());
    }

    #[test]
    // tests a small buffer growing only on second write
    fn write_1() {
        let mut buffer = Buffer::with_capacity(8);
        assert_eq!(buffer.len(), 0);
        assert_eq!(buffer.available_capacity(), 8);
        assert!(buffer.is_empty());

        // first write fits in buffer
        buffer.extend_from_slice(b"GET ");
        assert_eq!(buffer.len(), 4);
        assert_eq!(buffer.available_capacity(), 4);
        assert!(!buffer.is_empty());
        let content: &[u8] = buffer.borrow();
        assert_eq!(content, b"GET ");

        // second write causes buffer to grow
        buffer.extend_from_slice(b"SOME_KEY\r\n");
        assert_eq!(buffer.len(), 14);
        assert_eq!(buffer.available_capacity(), 2);
        assert!(!buffer.is_empty());
        let content: &[u8] = buffer.borrow();
        assert_eq!(content, b"GET SOME_KEY\r\n");
    }

    #[test]
    // test a zero capacity buffer growing on two consecutive writes
    fn write_2() {
        let mut buffer = Buffer::with_capacity(0);
        assert_eq!(buffer.len(), 0);
        assert_eq!(buffer.available_capacity(), 0);
        assert!(buffer.is_empty());

        // zero capacity buffer grows on first write
        buffer.extend_from_slice(b"GET KEY\r\n");
        assert_eq!(buffer.len(), 9);
        assert_eq!(buffer.available_capacity(), 7);
        assert!(!buffer.is_empty());
        let content: &[u8] = buffer.borrow();
        assert_eq!(content, b"GET KEY\r\n");

        // and again on second write
        buffer.extend_from_slice(b"SET OTHER_KEY 0 0 1\r\nA\r\n");
        assert_eq!(buffer.len(), 33);
        assert_eq!(buffer.available_capacity(), 31);
        assert!(!buffer.is_empty());
        let content: &[u8] = buffer.borrow();
        assert_eq!(content, b"GET KEY\r\nSET OTHER_KEY 0 0 1\r\nA\r\n");
    }

    #[test]
    // tests a large buffer that grows on first write
    fn write_3() {
        let mut buffer = Buffer::with_capacity(16);
        assert_eq!(buffer.len(), 0);
        assert_eq!(buffer.available_capacity(), 16);
        assert!(buffer.is_empty());

        buffer.extend_from_slice(b"SET SOME_REALLY_LONG_KEY 0 0 1\r\nA\r\n");
        assert_eq!(buffer.len(), 35);
        assert_eq!(buffer.available_capacity(), 29);
    }

    #[test]
    // tests a consume operation where all bytes are consumed and the buffer
    // remains its original size
    fn consume_1() {
        let mut buffer = Buffer::with_capacity(16);
        assert_eq!(buffer.len(), 0);
        assert_eq!(buffer.available_capacity(), 16);
        assert!(buffer.is_empty());

        buffer.extend_from_slice(b"END\r\n");
        assert_eq!(buffer.len(), 5);
        assert_eq!(buffer.available_capacity(), 11);
        assert!(!buffer.is_empty());

        buffer.consume(5);
        assert_eq!(buffer.len(), 0);
        assert_eq!(buffer.available_capacity(), 16);
        assert!(buffer.is_empty());
    }

    #[test]
    // tests a consume operation where all bytes are consumed and the buffer
    // shrinks to its original size
    fn consume_2() {
        let mut buffer = Buffer::with_capacity(2);
        assert_eq!(buffer.len(), 0);
        assert_eq!(buffer.available_capacity(), 2);
        assert!(buffer.is_empty());

        // buffer extends to the next power of two
        // with 5 byte message we need 8 bytes for the buffer
        buffer.extend_from_slice(b"END\r\n");
        assert_eq!(buffer.len(), 5);
        assert_eq!(buffer.available_capacity(), 3);
        assert!(!buffer.is_empty());

        buffer.consume(5);
        assert_eq!(buffer.len(), 0);
        assert_eq!(buffer.available_capacity(), 2);
        assert!(buffer.is_empty());
    }

    #[test]
    // tests a consume operation where not all bytes are consumed and buffer
    // remains its original size
    fn consume_3() {
        let mut buffer = Buffer::with_capacity(8);
        assert_eq!(buffer.len(), 0);
        assert_eq!(buffer.available_capacity(), 8);
        assert!(buffer.is_empty());

        let content = b"END\r\n";
        let len = content.len();

        buffer.extend_from_slice(content);
        assert_eq!(buffer.len(), len);
        assert_eq!(buffer.available_capacity(), 3);
        assert!(!buffer.is_empty());

        // consume all but the last byte of content in the buffer, one byte at
        // a time
        // - buffer len decreases with each call to consume()
        // - buffer available capacity remains the same
        for i in 1..len {
            buffer.consume(1);
            assert_eq!(buffer.len(), len - i);
            assert_eq!(buffer.available_capacity(), 3 + i);
            assert!(!buffer.is_empty());
        }

        // when consuming the final byte, the read/write offsets move to the
        // start of the buffer, and available capacity should be the original
        // buffer size
        buffer.consume(1);
        assert_eq!(buffer.len(), 0);
        assert_eq!(buffer.available_capacity(), 8);
        assert!(buffer.is_empty());
    }

    #[test]
    // tests a consume operation where not all bytes are consumed and buffer
    // shrinks as bytes are consumed
    fn consume_4() {
        let mut buffer = Buffer::with_capacity(16);
        assert_eq!(buffer.len(), 0);
        assert_eq!(buffer.available_capacity(), 16);
        assert!(buffer.is_empty());

        let content = b"VALUE SOME_REALLY_LONG_KEY 0 1\r\n1\r\nEND\r\n";

        // buffer resizes up to 64 bytes to hold 40 bytes
        // length = 40, size = 64, capacity = 24
        buffer.extend_from_slice(content);
        assert_eq!(buffer.len(), 40);
        assert_eq!(buffer.available_capacity(), 24);
        assert!(!buffer.is_empty());

        // partial consume, len decrease, buffer shrinks by half
        // length = 32, size = 32, capacity = 0
        buffer.consume(8);
        assert_eq!(buffer.len(), 32);
        assert_eq!(buffer.available_capacity(), 0);
        assert!(!buffer.is_empty());

        // consume one more byte and we should get available capacity
        // length = 31, size = 32, capacity = 1
        buffer.consume(1);
        assert_eq!(buffer.len(), 31);
        assert_eq!(buffer.available_capacity(), 1);
        assert!(!buffer.is_empty());

        // partial consume, len decrease, capacity remains the same
        // length = 16, size = 16, capacity = 0
        buffer.consume(15);
        assert_eq!(buffer.len(), 16);
        assert_eq!(buffer.available_capacity(), 0);

        // consume one more byte and the buffer shrinks because we have less
        // than half occupancy
        // length = 15, size = 16, capacity = 0
        buffer.consume(1);
        assert_eq!(buffer.len(), 15);
        assert_eq!(buffer.available_capacity(), 1);

        // partial consume, len decrease
        // length = 8, size = 16, capacity = 8
        buffer.consume(7);
        assert_eq!(buffer.len(), 8);
        assert_eq!(buffer.available_capacity(), 8);

        // partial consume, len decrease
        // length = 7, size = 16, capacity = 9
        buffer.consume(1);
        assert_eq!(buffer.len(), 7);
        assert_eq!(buffer.available_capacity(), 9);

        // consume all but the final byte
        // partial consume, len decrease
        // length = 1, size = 16, capacity = 15
        buffer.consume(6);
        assert_eq!(buffer.len(), 1);
        assert_eq!(buffer.available_capacity(), 15);

        // consume the final byte
        // length = 0, size = 16, capacity = 16
        buffer.consume(1);
        assert_eq!(buffer.len(), 0);
        assert_eq!(buffer.available_capacity(), 16);
    }
}
