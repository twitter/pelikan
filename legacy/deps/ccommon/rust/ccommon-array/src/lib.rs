// Copyright 2020 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

// constants to define default values
const NELEM_DELTA: usize = 16;

// helper functions
fn nelem_delta() -> usize {
    NELEM_DELTA
}

// definitions
#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ArrayConfig {
    #[cfg_attr(feature = "serde", serde(default = "nelem_delta"))]
    nelem_delta: usize,
}

// implementation
impl ArrayConfig {
    pub fn nelem_delta(&self) -> usize {
        self.nelem_delta
    }
}

// trait implementations
impl Default for ArrayConfig {
    fn default() -> Self {
        Self {
            nelem_delta: nelem_delta(),
        }
    }
}
