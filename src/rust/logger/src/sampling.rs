// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::*;
use core::sync::atomic::{AtomicUsize, Ordering};

/// Implements a logger which only logs 1 in N log messages.
pub(crate) struct SamplingLogger {
    logger: Logger,
    counter: AtomicUsize,
    sample: usize,
}

impl SamplingLogger {
    pub fn level_filter(&self) -> LevelFilter {
        self.logger.level_filter()
    }
}

impl Log for SamplingLogger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() <= self.level_filter()
    }

    fn log(&self, record: &log::Record<'_>) {
        // If the log message is filtered by the log level, return early.
        if !self.enabled(record.metadata()) {
            return;
        }

        let count = self.counter.fetch_add(1, Ordering::Relaxed);

        // if this is the Nth message, we should log it
        if (count % self.sample) == 0 {
            self.counter.fetch_sub(self.sample, Ordering::Relaxed);
            self.logger.log(record)
        } else {
            LOG_SKIP.increment();
        }
    }

    fn flush(&self) {}
}

/// A type to construct a basic `AsyncLog` which routes 1 in N log messages to a
/// single `Output`.
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

    /// Sets the depth of the log queue. Deeper queues are less likely to drop
    /// messages, but come at the cost of additional memory utilization.
    pub fn log_queue_depth(mut self, messages: usize) -> Self {
        self.log_builder = self.log_builder.log_queue_depth(messages);
        self
    }

    /// Sets the buffer size for a single message. Oversized messages will
    /// result in an extra allocation, but keeping this small allows deeper
    /// queues for the same total buffer size without dropping log messages.
    pub fn single_message_size(mut self, bytes: usize) -> Self {
        self.log_builder = self.log_builder.single_message_size(bytes);
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

    /// Consumes the builder and returns a configured `SamplingLogger` and `LogDrain`.
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

    /// Consumes the builder and returns an `AsyncLog`.
    pub fn build(self) -> Result<AsyncLog, &'static str> {
        let (logger, drain) = self.build_raw()?;
        let level_filter = logger.level_filter();
        Ok(AsyncLog {
            logger: Box::new(logger),
            drain: Box::new(drain),
            level_filter,
        })
    }
}
