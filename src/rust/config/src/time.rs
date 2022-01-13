// Copyright 2020 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

pub use common::expiry::TimeType;
use serde::{Deserialize, Serialize};

// TODO(bmartin): set the default back to unix

// constants to define default values
pub const DEFAULT_TIME_TYPE: TimeType = TimeType::Memcache;

// helper functions
fn time_type() -> TimeType {
    DEFAULT_TIME_TYPE
}

// definitions
#[derive(Serialize, Deserialize, Debug)]
pub struct TimeConfig {
    #[serde(default = "time_type")]
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
