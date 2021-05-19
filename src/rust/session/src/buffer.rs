// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! A very simple buffer type that can be replaced in the future.

pub struct Buffer {
    pub inner: BytesMut,
}

use bytes::BytesMut;
use common::ExtendFromSlice;
use std::borrow::Borrow;

impl Buffer {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            inner: BytesMut::with_capacity(capacity),
        }
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn split_to(&mut self, index: usize) -> Self {
        Self {
            inner: self.inner.split_to(index),
        }
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
