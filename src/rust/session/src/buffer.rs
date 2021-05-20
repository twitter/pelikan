// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! A very simple buffer type that can be replaced in the future.

use std::borrow::Borrow;

use bytes::BytesMut;
use bytes::Buf;
use common::ExtendFromSlice;

/// A growable byte buffer
pub struct Buffer {
    pub inner: BytesMut,
}

impl Buffer {
    /// Create a new `Buffer` that can hold up to `capacity` bytes without
    /// re-allocating.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            inner: BytesMut::with_capacity(capacity),
        }
    }

    /// Return the number of bytes currently in the buffer.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Check if the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Advance the buffer by the specified number of bytes
    pub fn advance(&mut self, bytes: usize) {
        self.inner.advance(bytes)
    }
}

impl Borrow<[u8]> for Buffer {
    fn borrow(&self) -> &[u8] {
        self.inner.borrow()
    }
}

impl ExtendFromSlice<u8> for Buffer {
    fn extend(&mut self, src: &[u8]) {
        self.inner.extend_from_slice(src)
    }
}
