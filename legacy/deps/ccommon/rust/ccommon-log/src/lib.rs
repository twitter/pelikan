// Copyright 2020 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

// constants to define default values
const DEBUG_LOG_LEVEL: usize = 4;
const DEBUG_LOG_FILE: Option<String> = None;
const DEBUG_LOG_NBUF: usize = 0;

// helper functions
fn log_level() -> usize {
    DEBUG_LOG_LEVEL
}

fn log_file() -> Option<String> {
    DEBUG_LOG_FILE
}

fn log_nbuf() -> usize {
    DEBUG_LOG_NBUF
}

// struct definitions
#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct DebugConfig {
    #[cfg_attr(feature = "serde", serde(default = "log_level"))]
    log_level: usize,
    #[cfg_attr(feature = "serde", serde(default = "log_file"))]
    log_file: Option<String>,
    #[cfg_attr(feature = "serde", serde(default = "log_nbuf"))]
    log_nbuf: usize,
}

// implementation
impl DebugConfig {
    pub fn log_level(&self) -> usize {
        self.log_level
    }

    pub fn log_file(&self) -> Option<String> {
        self.log_file.clone()
    }

    pub fn log_nbuf(&self) -> usize {
        self.log_nbuf
    }
}

// trait implementations
impl Default for DebugConfig {
    fn default() -> Self {
        Self {
            log_level: log_level(),
            log_file: log_file(),
            log_nbuf: log_nbuf(),
        }
    }
}
