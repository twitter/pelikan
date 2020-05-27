// Copyright 2020 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

// constants to define default values
const BUFSOCK_POOLSIZE: usize = 0;

// helper functions
fn buf_sock_poolsize() -> usize {
    BUFSOCK_POOLSIZE
}

// definitions
#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct SockioConfig {
	#[cfg_attr(feature = "serde", serde(default = "buf_sock_poolsize"))]
    buf_sock_poolsize: usize,
}

// implementation
impl SockioConfig {
    pub fn buf_sock_poolsize(&self) -> usize {
        self.buf_sock_poolsize
    }
}

// trait implementations
impl Default for SockioConfig {
    fn default() -> Self {
        Self {
            buf_sock_poolsize: buf_sock_poolsize(),
        }
    }
}
