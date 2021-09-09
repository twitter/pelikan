// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::units::*;
use serde::{Deserialize, Serialize};

////////////////////////////////////////////////////////////////////////////////
// constants to define default values
////////////////////////////////////////////////////////////////////////////////

// log to the file path
const FILE: Option<String> = None;

// log will rotate to the given backup path
const BACKUP: Option<String> = None;

// flush interval in milliseconds
const INTERVAL: usize = 100;

// max log size before rotate in bytes
const MAX_SIZE: u64 = GB as u64;

// buffer size in bytes
const NBUF: usize = 0;

// log 1 in every N commands
const SAMPLE: usize = 100;

////////////////////////////////////////////////////////////////////////////////
// helper functions
////////////////////////////////////////////////////////////////////////////////

fn file() -> Option<String> {
    FILE
}

fn backup() -> Option<String> {
    BACKUP
}

fn interval() -> usize {
    INTERVAL
}

fn max_size() -> u64 {
    MAX_SIZE
}

fn nbuf() -> usize {
    NBUF
}

fn sample() -> usize {
    SAMPLE
}

////////////////////////////////////////////////////////////////////////////////
// struct definitions
////////////////////////////////////////////////////////////////////////////////

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct KlogConfig {
    #[serde(default = "file")]
    file: Option<String>,
    #[serde(default = "backup")]
    backup: Option<String>,
    #[serde(default = "interval")]
    interval: usize,
    #[serde(default = "nbuf")]
    nbuf: usize,
    #[serde(default = "sample")]
    sample: usize,
    #[serde(default = "max_size")]
    max_size: u64,
}

////////////////////////////////////////////////////////////////////////////////
// implementation
////////////////////////////////////////////////////////////////////////////////

impl KlogConfig {
    pub fn file(&self) -> Option<String> {
        self.file.clone()
    }

    pub fn backup(&self) -> Option<String> {
        match &self.backup {
            Some(path) => Some(path.clone()),
            None => self.file.as_ref().map(|path| format!("{}.old", path)),
        }
    }

    pub fn interval(&self) -> usize {
        self.interval
    }

    pub fn max_size(&self) -> u64 {
        self.max_size
    }

    pub fn nbuf(&self) -> usize {
        self.nbuf
    }

    pub fn sample(&self) -> usize {
        self.sample
    }
}

// trait implementations
impl Default for KlogConfig {
    fn default() -> Self {
        Self {
            file: file(),
            backup: backup(),
            interval: interval(),
            max_size: max_size(),
            nbuf: nbuf(),
            sample: sample(),
        }
    }
}
