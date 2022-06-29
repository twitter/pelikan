// Copyright 2020 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

// constants to define default values
const STATS_LOG_FILE: Option<String> = None;
const STATS_LOG_NBUF: usize = 0;

// helper functions
fn file() -> Option<String> {
    STATS_LOG_FILE
}

fn nbuf() -> usize {
    STATS_LOG_NBUF
}

// definitions
#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct StatsLogConfig {
	#[cfg_attr(feature = "serde", serde(default = "file"))]
    file: Option<String>,
    #[cfg_attr(feature = "serde", serde(default = "nbuf"))]
    nbuf: usize,
}

// implementation
impl StatsLogConfig {
    pub fn log_file(&self) -> Option<String> {
        self.file.clone()
    }

    pub fn log_nbuf(&self) -> usize {
        self.nbuf
    }
}

// trait implementations
impl Default for StatsLogConfig {
    fn default() -> Self {
        Self {
            file: file(),
            nbuf: nbuf(),
        }
    }
}
