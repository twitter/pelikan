// Copyright 2020 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use serde::{Deserialize, Serialize};

// constants to define default values
const DBUF_DEFAULT_MAX: usize = 8;

// helper functions
fn max_power() -> usize {
    DBUF_DEFAULT_MAX
}

// struct definitions
#[derive(Serialize, Deserialize, Debug)]
pub struct Dbuf {
    #[serde(default = "max_power")]
    max_power: usize,
}

// implementation
impl Dbuf {
    pub fn max_power(&self) -> usize {
        self.max_power
    }
}

pub trait DbufConfig {
    fn dbuf(&self) -> &Dbuf;
}

// trait implementations
impl Default for Dbuf {
    fn default() -> Self {
        Self {
            max_power: max_power(),
        }
    }
}
