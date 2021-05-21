// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

#[macro_use]
extern crate rustcommon_logger;

#[macro_use]
extern crate rustcommon_fastmetrics;

mod process;
mod threads;

pub use process::{Process, ProcessBuilder};

const DEFAULT_BUFFER_SIZE: usize = 1024; // 1KB
