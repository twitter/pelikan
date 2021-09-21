// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use core::sync::atomic::{AtomicUsize, Ordering};
use crate::*;

/// The core of the logging backend, this type is registered as the global
/// logger and is compatible with the `log` crate's macros. Instead of directly
/// logging, it enqueues the formatted log message. Users will typically not
/// interact with this type directly, but would work with the `LogHandle`.
pub(crate) struct SamplingLogger {
    logger: Logger,
    counter: AtomicUsize,
    sample: usize,
}

impl LogEx for SamplingLogger {
    fn level_filter(&self) -> LevelFilter {
        self.logger.level_filter()
    }
}

impl Log for SamplingLogger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() <= self.logger.level_filter()
    }

    fn log(&self, record: &log::Record<'_>) {
        // If the log message is filtered by the log level, return early.
        if !self.enabled(record.metadata()) {
            return;
        }

        // TODO(bmartin): double check logic here

        // if this is the Nth message, we should log it
        if self.counter.fetch_add(1, Ordering::Relaxed) == self.sample {
            self.counter.fetch_sub(self.sample, Ordering::Relaxed);
            self.logger.log(record)
        }
    }

    fn flush(&self) {}
}


/// A type to construct a `Logger` and `LogDrain` pair.
pub struct SamplingLogBuilder {
    log_builder: LogBuilder,
    sample: usize,
}

impl Default for SamplingLogBuilder {
    fn default() -> Self {
        Self {
            log_builder: LogBuilder::default(),
            sample: 100,
        }
    }
}

impl SamplingLogBuilder {
    /// Create a new log builder.
    pub fn new() -> Self {
        Default::default()
    }

    /// Sets the total buffer size for log messages.
    pub fn total_buffer_size(mut self, bytes: usize) -> Self {
        self.log_builder = self.log_builder.total_buffer_size(bytes);
        self
    }

    /// Sets the log message buffer size. Oversized messages will result in an
    /// extra allocation, but keeping this small allows deeper queues for the
    /// same total buffer size without dropping log messages.
    pub fn log_message_size(mut self, bytes: usize) -> Self {
        self.log_builder = self.log_builder.log_message_size(bytes);
        self
    }

    /// Sets the output for the logger.
    pub fn output(mut self, output: Box<dyn Output>) -> Self {
        self.log_builder = self.log_builder.output(output);
        self
    }

    /// Sets the format function to be used to format messages to this log.
    pub fn format(mut self, format: FormatFunction) -> Self {
        self.log_builder = self.log_builder.format(format);
        self
    }

    /// Sets the sampling to 1 in N requests
    pub fn sample(mut self, sample: usize) -> Self {
        self.sample = sample;
        self
    }

    /// Consumes the builder and returns a configured `Logger` and `LogHandle`.
    pub(crate) fn build_raw(self) -> Result<(SamplingLogger, LogDrain), &'static str> {
        let (logger, log_handle) = self.log_builder.build_raw()?;
        let logger = SamplingLogger {
            logger,
            // initialize to 1 not 0 so the first fetch_add returns a 1
            counter: AtomicUsize::new(1),
            sample: self.sample,
        };
        Ok((logger, log_handle))
    }

    /// Consumes the builder and returns a configured `Box<dyn Log>` and `Box<dyn Drain>`.
    pub fn build(self) -> Result<AsyncLog, &'static str> {
        let (logger, drain) = self.build_raw()?;
        Ok(AsyncLog { logger: Box::new(logger), drain: Box::new(drain) })
    }
}
