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

// logger queue depth
const QUEUE_DEPTH: usize = 4096;

// log 1 in every N commands
const SAMPLE: usize = 100;

// single message buffer size in bytes
const SINGLE_MESSAGE_SIZE: usize = KB;

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

fn queue_depth() -> usize {
    QUEUE_DEPTH
}

fn sample() -> usize {
    SAMPLE
}

fn single_message_size() -> usize {
    SINGLE_MESSAGE_SIZE
}

////////////////////////////////////////////////////////////////////////////////
// struct definitions
////////////////////////////////////////////////////////////////////////////////

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Klog {
    #[serde(default = "backup")]
    backup: Option<String>,
    #[serde(default = "file")]
    file: Option<String>,
    #[serde(default = "interval")]
    interval: usize,
    #[serde(default = "max_size")]
    max_size: u64,
    #[serde(default = "queue_depth")]
    queue_depth: usize,
    #[serde(default = "sample")]
    sample: usize,
    #[serde(default = "single_message_size")]
    single_message_size: usize,
}

////////////////////////////////////////////////////////////////////////////////
// implementation
////////////////////////////////////////////////////////////////////////////////

impl Klog {
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

    pub fn queue_depth(&self) -> usize {
        self.queue_depth
    }

    pub fn sample(&self) -> usize {
        self.sample
    }

    pub fn single_message_size(&self) -> usize {
        self.single_message_size
    }
}

// trait implementations
impl Default for Klog {
    fn default() -> Self {
        Self {
            file: file(),
            backup: backup(),
            interval: interval(),
            max_size: max_size(),
            queue_depth: queue_depth(),
            sample: sample(),
            single_message_size: single_message_size(),
        }
    }
}

// trait definitions
pub trait KlogConfig {
    fn klog(&self) -> &Klog;
}
