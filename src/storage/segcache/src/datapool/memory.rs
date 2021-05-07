// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! A simple memory backed datapool which stores a contiguous slice of bytes
//! heap-allocated in main memory.

use crate::datapool::Datapool;

/// A contiguous allocation of bytes in main memory
pub struct Memory {
    data: Box<[u8]>,
}

impl Memory {
    /// Create a new `Memory` datapool with the specified size (in bytes)
    pub fn create(size: usize) -> Self {
        let data = vec![0; size];
        let data = data.into_boxed_slice();

        Self { data }
    }
}

impl Datapool for Memory {
    fn as_slice(&self) -> &[u8] {
        &self.data
    }

    fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.data
    }

    fn flush(&self) -> Result<(), std::io::Error> {
        Ok(())
    }
}
