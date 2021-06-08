// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! A very simple buffer type that can be replaced in the future.

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

        Self {
            buffer,
            read_offset: 0,
            write_offset: 0,
            target_capacity: capacity,
        }
    }

    /// Return the number of bytes currently in the buffer.
    pub fn len(&self) -> usize {
        self.write_offset - self.read_offset
    }

    /// Check if the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn reserve(&mut self, additional: usize) {
        self.buffer.reserve(additional);
        self.buffer.resize(self.buffer.capacity(), 0);
    }

    pub fn extend_from_slice(&mut self, other: &[u8]) {
        self.buffer.resize(self.write_offset + other.len(), 0);
        self.buffer[self.write_offset..(self.write_offset + other.len())].copy_from_slice(other);
        self.increase_len(other.len());
    }

    pub fn consume(&mut self, amt: usize) {
        self.read_offset = std::cmp::min(self.read_offset + amt, self.write_offset);
        if self.is_empty() {
            self.write_offset = 0;
            self.read_offset = 0;
            if self.buffer.len() > self.target_capacity {
                self.buffer.truncate(self.target_capacity);
                self.buffer.shrink_to_fit();
            }
        } else if self.read_offset > self.target_capacity {
            self.buffer.copy_within(self.read_offset..self.write_offset, 0);
            self.write_offset -= self.read_offset;
            self.read_offset = 0;
        }
    }

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
