// Copyright 2020 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

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
#[derive(Serialize, Deserialize, Debug)]
pub struct StatsLog {
    #[serde(default = "file")]
    file: Option<String>,
    #[serde(default = "nbuf")]
    nbuf: usize,
}

// implementation
impl StatsLog {
    pub fn log_file(&self) -> Option<String> {
        self.file.clone()
    }

    pub fn log_nbuf(&self) -> usize {
        self.nbuf
    }
}

// trait definitions
pub trait StatsLogConfig {
    fn stats_log(&self) -> &StatsLog;
}

// trait implementations
impl Default for StatsLog {
    fn default() -> Self {
        Self {
            file: file(),
            nbuf: nbuf(),
        }
    }
}
