// Copyright 2020 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use serde::{Deserialize, Serialize};

// constants to define default values
const BUFSOCK_POOLSIZE: usize = 0;

// helper functions
fn buf_sock_poolsize() -> usize {
    BUFSOCK_POOLSIZE
}

// definitions
#[derive(Serialize, Deserialize, Debug)]
pub struct Sockio {
    #[serde(default = "buf_sock_poolsize")]
    buf_sock_poolsize: usize,
}

// implementation
impl Sockio {
    pub fn buf_sock_poolsize(&self) -> usize {
        self.buf_sock_poolsize
    }
}

// trait implementations
impl Default for Sockio {
    fn default() -> Self {
        Self {
            buf_sock_poolsize: buf_sock_poolsize(),
        }
    }
}

// trait definitions
pub trait SockioConfig {
    fn sockio(&self) -> &Sockio;
}
