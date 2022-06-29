// Copyright 2020 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

// constants to define default values
const TCP_BACKLOG: usize = 128;
const TCP_POOLSIZE: usize = 0;

// helper functions
fn backlog() -> usize {
    TCP_BACKLOG
}

fn poolsize() -> usize {
    TCP_POOLSIZE
}

// definitions
#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct TcpConfig {
	#[cfg_attr(feature = "serde", serde(default = "backlog"))]
    backlog: usize,
    #[cfg_attr(feature = "serde", serde(default = "poolsize"))]
    poolsize: usize,
}

// implementation
impl TcpConfig {
    pub fn backlog(&self) -> usize {
        self.backlog
    }

    pub fn poolsize(&self) -> usize {
        self.poolsize
    }
}

// trait implementations
impl Default for TcpConfig {
    fn default() -> Self {
        Self {
            backlog: backlog(),
            poolsize: poolsize(),
        }
    }
}
