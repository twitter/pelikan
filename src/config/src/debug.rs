// Copyright 2020 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::units::*;
use log::Level;
use serde::{Deserialize, Serialize};

// constants to define default values
const LOG_LEVEL: Level = Level::Info;
const LOG_FILE: Option<String> = None;
const LOG_BACKUP: Option<String> = None;
const LOG_MAX_SIZE: u64 = GB as u64;
const LOG_QUEUE_DEPTH: usize = 4096;
const LOG_SINGLE_MESSAGE_SIZE: usize = KB;

// helper functions
fn log_level() -> Level {
    LOG_LEVEL
}

fn log_file() -> Option<String> {
    LOG_FILE
}

fn log_backup() -> Option<String> {
    LOG_BACKUP
}

fn log_max_size() -> u64 {
    LOG_MAX_SIZE
}

fn log_queue_depth() -> usize {
    LOG_QUEUE_DEPTH
}

fn log_single_message_size() -> usize {
    LOG_SINGLE_MESSAGE_SIZE
}

// struct definitions
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Debug {
    #[serde(with = "LevelDef")]
    #[serde(default = "log_level")]
    log_level: Level,
    #[serde(default = "log_file")]
    log_file: Option<String>,
    #[serde(default = "log_backup")]
    log_backup: Option<String>,
    #[serde(default = "log_max_size")]
    log_max_size: u64,
    #[serde(default = "log_queue_depth")]
    log_queue_depth: usize,
    #[serde(default = "log_single_message_size")]
    log_single_message_size: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
#[serde(remote = "Level")]
#[serde(deny_unknown_fields)]
enum LevelDef {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

// implementation
impl Debug {
    pub fn log_level(&self) -> Level {
        self.log_level
    }

    pub fn log_file(&self) -> Option<String> {
        self.log_file.clone()
    }

    pub fn log_backup(&self) -> Option<String> {
        match &self.log_backup {
            Some(path) => Some(path.clone()),
            None => self.log_file.as_ref().map(|path| format!("{}.old", path)),
        }
    }

    pub fn log_max_size(&self) -> u64 {
        self.log_max_size
    }

    pub fn log_queue_depth(&self) -> usize {
        self.log_queue_depth
    }

    pub fn log_single_message_size(&self) -> usize {
        self.log_single_message_size
    }
}

// trait implementations
impl Default for Debug {
    fn default() -> Self {
        Self {
            log_level: log_level(),
            log_file: log_file(),
            log_backup: log_backup(),
            log_max_size: log_max_size(),
            log_queue_depth: log_queue_depth(),
            log_single_message_size: log_single_message_size(),
        }
    }
}

// trait definitions
pub trait DebugConfig {
    fn debug(&self) -> &Debug;
}
