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

    /// Returns the amount of space available to write into the buffer without
    /// reallocating.
    pub fn available_capacity(&self) -> usize {
        self.buffer.len() - self.write_offset
    }

    pub fn capacity(&self) -> usize {
        self.buffer.len()
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
        let needed = additional.saturating_sub(self.available_capacity());
        if needed > 0 {
            self.buffer.reserve(needed);
            self.buffer.resize(self.buffer.capacity(), 0);
        }
    }

    /// Append the bytes from `other` onto `self`.
    pub fn extend_from_slice(&mut self, other: &[u8]) {
        self.buffer.resize(self.write_offset + other.len(), 0);
        self.buffer[self.write_offset..(self.write_offset + other.len())].copy_from_slice(other);
        self.increase_len(other.len());
    }

    /// Mark that `amt` bytes have been consumed and should not be returned in
    /// future reads from the buffer.
    pub fn consume(&mut self, amt: usize) {
        self.read_offset = std::cmp::min(self.read_offset + amt, self.write_offset);
        if self.is_empty() {
            // if the buffer is empty, we can simply shrink it down and move the
            // offsets to the start of the buffer storage
            self.write_offset = 0;
            self.read_offset = 0;
            if self.buffer.len() > self.target_capacity {
                self.buffer.truncate(self.target_capacity);
                self.buffer.shrink_to_fit();
            }
        } else if self.read_offset > self.target_capacity {
            // this case results in a memmove of the buffer contents to the
            // beginning of the buffer storage and tries to free additional
            // space
            self.buffer
                .copy_within(self.read_offset..self.write_offset, 0);
            self.write_offset -= self.read_offset;
            self.read_offset = 0;
            // if the buffer is occupying less than 1/2 of the storage capacity
            // we can resize it to free up the unused space at the end of the
            // buffer storage.
            if self.len() < self.buffer.capacity() / 2 {
                self.buffer
                    .truncate(std::cmp::max(self.len(), self.target_capacity));
                self.buffer.resize(self.buffer.capacity(), 0);
            }
        }
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
