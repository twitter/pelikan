// Copyright 2020 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

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
#[derive(Serialize, Deserialize, Debug)]
pub struct TcpConfig {
    #[serde(default = "backlog")]
    backlog: usize,
    #[serde(default = "poolsize")]
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
