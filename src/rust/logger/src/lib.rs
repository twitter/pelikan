// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! This crate provides logging functionality that is focused on both normal log
//! messages through the usual `log` macros, as well as command logging using
//! the custom `klog!()` macro.
//!
//! This logging crate focuses on minimizing the costs at the callsite and
//! relying on a thread outside of the critical path to handle any IO, whether
//! to a file or to standard out. This crate realizes this goal by sending the
//! log messages over a queue to a `LogReceiver`. It is expected that this
//! struct will be flushed periodically to free up room in the queue.

use core::sync::atomic::{AtomicUsize, Ordering};
use rustcommon_time::recent_local;
use std::path::Path;

use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::Arc;

use config::{DebugConfig, KlogConfig};
use mpmc::Queue;

pub use log::*;

mod backend;
mod builder;
mod format;
mod macros;
mod receiver;
mod sender;

pub(crate) use backend::*;

pub use builder::*;
pub use format::*;
pub use macros::*;
pub use receiver::*;
pub use sender::*;

const KB: usize = 1024;
const MB: usize = 1024 * KB;

const DEFAULT_MSG_SIZE: usize = KB;
const DEFAULT_BUFFER_SIZE: usize = 2 * MB;
