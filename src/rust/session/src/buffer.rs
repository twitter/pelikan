// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! A very simple buffer type that can be replaced in the future.

use core::borrow::{Borrow, BorrowMut};

/// A growable byte buffer
pub struct Buffer {
    buffer: Vec<u8>,
    position: usize,
    capacity: usize,
}

impl Buffer {
    /// Create a new `Buffer` that can hold up to `capacity` bytes without
    /// re-allocating.
    pub fn with_capacity(capacity: usize) -> Self {
        let buffer = vec![0; capacity];
        // let buffer = buffer.into_boxed_slice();

        Self {
            buffer,
            capacity: 0,
            position: 0,
        }
    }

    /// Return the number of bytes currently in the buffer.
    pub fn len(&self) -> usize {
        self.capacity - self.position
    }

    /// Check if the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn reserve(&mut self, additional: usize) {
        self.buffer.reserve(additional);
    }

    pub fn extend_from_slice(&mut self, other: &[u8]) {
        self.buffer.reserve(other.len());
        self.buffer[self.capacity..(self.capacity + other.len())].copy_from_slice(other);
        self.increase_len(other.len());
    }

    pub fn consume(&mut self, amt: usize) {
        self.position = std::cmp::min(self.position + amt, self.capacity);
        if self.is_empty() {
            self.capacity = 0;
            self.position = 0;
        }
    }

    pub fn increase_len(&mut self, amt: usize) {
        self.capacity = std::cmp::min(self.capacity + amt, self.buffer.len());
    }
}

impl Borrow<[u8]> for Buffer {
    fn borrow(&self) -> &[u8] {
        &self.buffer[self.position..self.capacity]
    }
}

impl BorrowMut<[u8]> for Buffer {
    fn borrow_mut(&mut self) -> &mut [u8] {
        let available = self.buffer.len();
        &mut self.buffer[self.capacity..available]
    }
}
