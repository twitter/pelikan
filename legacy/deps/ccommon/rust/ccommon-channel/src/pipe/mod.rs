// Copyright 2020 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

// constants to define default values
const PIPE_POOLSIZE: usize = 0;

// helper functions
fn poolsize() -> usize {
    PIPE_POOLSIZE
}

// definitions
#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct PipeConfig {
    #[cfg_attr(feature = "serde", serde(default = "poolsize"))]
    poolsize: usize,
}

// implementation
impl PipeConfig {
    pub fn poolsize(&self) -> usize {
        self.poolsize
    }
}

// trait implementations
impl Default for PipeConfig {
    fn default() -> Self {
        Self {
            poolsize: poolsize(),
        }
    }
}
