// Copyright 2020 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

// constants to define default values
const DBUF_DEFAULT_MAX: usize = 8;

// helper functions
fn max_power() -> usize {
    DBUF_DEFAULT_MAX
}

// struct definitions
#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct DbufConfig {
    #[cfg_attr(feature = "serde", serde(default = "max_power"))]
    max_power: usize,
}

// implementation
impl DbufConfig {
    pub fn max_power(&self) -> usize {
        self.max_power
    }
}

// trait implementations
impl Default for DbufConfig {
    fn default() -> Self {
        Self {
            max_power: max_power(),
        }
    }
}
