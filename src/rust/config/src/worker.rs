// Copyright 2020 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use serde::{Deserialize, Serialize};

// constants to define default values
const WORKER_TIMEOUT: usize = 100;
const WORKER_NEVENT: usize = 1024;

// helper functions
fn timeout() -> usize {
    WORKER_TIMEOUT
}

fn nevent() -> usize {
    WORKER_NEVENT
}

// definitions
#[derive(Serialize, Deserialize, Debug)]
pub struct WorkerConfig {
    #[serde(default = "timeout")]
    timeout: usize,
    #[serde(default = "nevent")]
    nevent: usize,
}

// implementation
impl WorkerConfig {
    pub fn timeout(&self) -> usize {
        self.timeout
    }

    pub fn nevent(&self) -> usize {
        self.nevent
    }
}

// trait implementations
impl Default for WorkerConfig {
    fn default() -> Self {
        Self {
            timeout: timeout(),
            nevent: nevent(),
        }
    }
}
