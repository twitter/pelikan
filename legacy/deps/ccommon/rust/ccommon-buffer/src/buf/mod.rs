// Copyright 2020 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

// constants to define default values
const BUF_DEFAULT_SIZE: usize = 16 * 1024;
const BUF_POOLSIZE: usize = 0;

// helper functions
fn size() -> usize {
    BUF_DEFAULT_SIZE
}

fn poolsize() -> usize {
    BUF_POOLSIZE
}

// struct definitions
#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct BufConfig {
    #[cfg_attr(feature = "serde", serde(default = "size"))]
    size: usize,
    #[cfg_attr(feature = "serde", serde(default = "poolsize"))]
    poolsize: usize,
}

// implementation
impl BufConfig {
    pub fn size(&self) -> usize {
        self.size
    }

    pub fn poolsize(&self) -> usize {
        self.poolsize
    }
}

// trait implementations
impl Default for BufConfig {
    fn default() -> Self {
        Self {
            size: size(),
            poolsize: poolsize(),
        }
    }
}
