// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! A simple memory backed datapool which stores a contiguous slice of bytes
//! heap-allocated in main memory.

use crate::datapool::Datapool;

/// A contiguous allocation of bytes in main memory
#[derive(Clone)]
pub struct Memory {
    data: Box<[u8]>,
}

impl Memory {
    /// Create a new `Memory` datapool with the specified size (in bytes)
    pub fn create(size: usize, prefault: bool) -> Self {
        // We allow slow vector initialization here because it is necessary for
        // prefaulting the vector. If we use just the macro, the memory region
        // is allocated but will not become resident.
        #[allow(clippy::slow_vector_initialization)]
        let data = if prefault {
            // TODO(bmartin): this pattern can likely be replaced with the vec
            // macro + a read every page_size bytes which may be faster than the
            // resize pattern used here.
            let mut data = Vec::with_capacity(size);
            data.resize(size, 0);
            data
        } else {
            vec![0; size]
        };

        let data = data.into_boxed_slice();

        Self { data }
    }

    // Used only in Segments::clone() in order to clone `Segments.data`
    #[cfg(test)]
    pub fn memory_from_data(data: Box<[u8]>) -> Memory {
        Memory { data }
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


impl From<Box<[u8]>> for Memory {
    fn from(data: Box<[u8]>) -> Memory {
        Memory { data }
    }
}
