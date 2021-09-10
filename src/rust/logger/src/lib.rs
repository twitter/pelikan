// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use rustcommon_time::recent_local;
use core::sync::atomic::{AtomicUsize, Ordering};
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
