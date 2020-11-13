// Copyright 2020 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

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
#[derive(Serialize, Deserialize, Debug)]
pub struct BufConfig {
    #[serde(default = "size")]
    size: usize,
    #[serde(default = "poolsize")]
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
