// Copyright 2020 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

// constants to define default values
const DEFAULT_TIME_TYPE: TimeType = TimeType::Unix;

// helper functions
fn time_type() -> TimeType {
    DEFAULT_TIME_TYPE
}

// definitions
#[derive(Copy, Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum TimeType {
    Unix = 0,
    Delta = 1,
    Memcache = 2,
    Sentinel = 3,
}

#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct TimeConfig {
    #[cfg_attr(feature = "serde", serde(default = "time_type"))]
    time_type: TimeType,
}

// implementation
impl TimeConfig {
    pub fn time_type(&self) -> TimeType {
        self.time_type
    }
}

// trait implementations
impl Default for TimeConfig {
    fn default() -> Self {
        Self {
            time_type: time_type(),
        }
    }
}
