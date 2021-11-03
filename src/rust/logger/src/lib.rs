// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! This crate provides an asynchronous logging backend that can direct logs to
//! one or more outputs.
//!
//! The core of this crate is the `AsyncLog` type, which is constructed using a
//! builder that is specific to your logging needs. After building the
//! `AsyncLog`, it can be registered as the global logger using the `start`
//! method. You will be left with a `Box<dyn Drain>` which should be
//! periodically flushed outside of any critical path. For example, in an admin
//! thread or dedicated logging thread.
//!
//! For logging to a single file, the `LogBuilder` type can be used to construct
//! an `AsyncLog` which has low overhead, but directs log messages to a single
//! `Output`.
//!
//! A `SamplingLogBuilder` can be used to construct an `AsyncLog` which will
//! filter the log messages using sampling before directing the log messages to
//! a single `Output`.
//!
//! A `MultiLogBuilder` can be used to construct an `AsyncLog` which routes log
//! messages based on the `target` metadata of the log `Record`. If there is an
//! `AsyncLog` registered for that specific `target`, then the log message will
//! be routed to that instance of `AsyncLog`. Log messages that do not match any
//! specific target will be routed to the default `AsyncLog` that has been added
//! to the `MultiLogBuilder`. If there is no default, messages that do not match
//! any specific target will be simply dropped.
//!
//! This combination of logging types allows us to compose a logging backend
//! which meets the application's needs. For example, you can use a local log
//! macro to set the target to some specific category and log those messages to
//! a file, while letting all other log messages pass to standard out. This
//! could allow splitting command/access/audit logs from the normal logging.

pub use log::*;

mod format;
mod multi;
mod nop;
mod outputs;
mod sampling;
mod single;
mod traits;

pub use format::*;
pub use multi::*;
pub use nop::*;
pub use outputs::*;
pub use sampling::*;
pub use single::*;
pub use traits::*;

use config::{DebugConfig, KlogConfig};
use metrics::{static_metrics, Counter, Gauge};
use mpmc::Queue;
use rustcommon_time::recent_utc;

pub(crate) type LogBuffer = Vec<u8>;

static_metrics! {
    static LOG_CREATE: Counter;
    static LOG_CREATE_EX: Counter;
    static LOG_DESTROY: Counter;
    static LOG_CURR: Gauge;
    static LOG_OPEN: Counter;
    static LOG_OPEN_EX: Counter;
    static LOG_WRITE: Counter;
    static LOG_WRITE_BYTE: Counter;
    static LOG_WRITE_EX: Counter;
    static LOG_SKIP: Counter;
    static LOG_DROP: Counter;
    static LOG_DROP_BYTE: Counter;
    static LOG_FLUSH: Counter;
    static LOG_FLUSH_EX: Counter;
}

/// A type which implements an asynchronous logging backend.
pub struct AsyncLog {
    pub(crate) logger: Box<dyn Log>,
    pub(crate) drain: Box<dyn Drain>,
    pub(crate) level_filter: LevelFilter,
}

impl AsyncLog {
    /// Register the logger and return a type which implements `Drain`. It is
    /// up to the user to periodically call flush on the resulting drain.
    pub fn start(self) -> Box<dyn Drain> {
        let level_filter = self.level_filter;
        log::set_boxed_logger(self.logger)
            .map(|()| log::set_max_level(level_filter))
            .expect("failed to start logger");
        self.drain
    }
}

////////////////////////////////////////////////////////////////////////////////
// TODO(bmartin): everything below is Pelikan specific, and should be factored
// out into a helper when we move this crate into rustcommon
////////////////////////////////////////////////////////////////////////////////

#[macro_export]
macro_rules! klog {
    ($($arg:tt)*) => (
        // we choose error level here because it is the lowest level and will
        // not be filtered unless the level filter is set to `off`
        error!(target: "klog", $($arg)*);
    )
}

pub fn configure_logging(debug_config: &DebugConfig, klog_config: &KlogConfig) -> Box<dyn Drain> {
    let debug_output: Box<dyn Output> = if let Some(file) = debug_config.log_file() {
        let backup = debug_config.log_backup().unwrap_or(format!("{}.old", file));
        Box::new(
            File::new(&file, &backup, debug_config.log_max_size())
                .expect("failed to open debug log file"),
        )
    } else {
        Box::new(Stdout::new())
    };

    let debug_log = LogBuilder::new()
        .output(debug_output)
        .log_queue_depth(debug_config.log_queue_depth())
        .single_message_size(debug_config.log_single_message_size())
        .build()
        .expect("failed to initialize debug log");

    let klog = if let Some(file) = klog_config.file() {
        let backup = klog_config.backup().unwrap_or(format!("{}.old", file));
        let output = Box::new(
            File::new(&file, &backup, klog_config.max_size()).expect("failed to open klog file"),
        );
        SamplingLogBuilder::new()
            .output(output)
            .format(klog_format)
            .sample(klog_config.sample())
            .log_queue_depth(klog_config.queue_depth())
            .single_message_size(klog_config.single_message_size())
            .build()
            .expect("failed to initialize klog")
    } else {
        NopLogBuilder::new().build()
    };

    MultiLogBuilder::new()
        .level_filter(debug_config.log_level().to_level_filter())
        .default(debug_log)
        .add_target("klog", klog)
        .build()
        .start()
}

metrics::test_no_duplicates!();