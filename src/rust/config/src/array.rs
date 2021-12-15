// Copyright 2020 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use serde::{Deserialize, Serialize};

// constants to define default values
const NELEM_DELTA: usize = 16;

// helper functions
fn nelem_delta() -> usize {
    NELEM_DELTA
}

// definitions
#[derive(Serialize, Deserialize, Debug)]
pub struct Array {
    #[serde(default = "nelem_delta")]
    nelem_delta: usize,
}

// implementation
impl Array {
    pub fn nelem_delta(&self) -> usize {
        self.nelem_delta
    }
}

pub trait ArrayConfig {
    fn array(&self) -> &Array;
}

// trait implementations
impl Default for Array {
    fn default() -> Self {
        Self {
            nelem_delta: nelem_delta(),
        }
    }
}
