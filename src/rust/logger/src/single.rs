// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::*;
use std::io::{Error, Write};

/// Implements a basic logger which sends all log messages to a single queue.
pub(crate) struct Logger {
    log_filled: Queue<LogBuffer>,
    log_cleared: Queue<LogBuffer>,
    buffer_size: usize,
    format: FormatFunction,
    level_filter: LevelFilter,
}

impl Logger {
    pub fn level_filter(&self) -> LevelFilter {
        self.level_filter
    }
}

impl Log for Logger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() <= self.level_filter
    }

    fn log(&self, record: &log::Record<'_>) {
        // If the log message is filtered by the log level, return early.
        if !self.enabled(record.metadata()) {
            return;
        }

        // Tries to re-use a buffer from the pool or allocate a new buffer to
        // to avoid blocking and try to avoid dropping the message. Message may
        // still be dropped if the log_filled queue is full.
        let mut buffer = self
            .log_cleared
            .pop()
            .unwrap_or_else(|| Vec::with_capacity(self.buffer_size));

        // Write the log message into the buffer and send to the receiver
        if (self.format)(&mut buffer, recent_local(), record).is_ok() {
            // Note this may drop a log message, but avoids blocking.
            let _ = self.log_filled.push(buffer);
        }
    }

    fn flush(&self) {}
}

/// Implements a basic drain type which receives log messages over a queue and
/// flushes them to a single buffered output.
pub(crate) struct LogDrain {
    log_filled: Queue<LogBuffer>,
    log_cleared: Queue<LogBuffer>,
    buffer_size: usize,
    output: Box<dyn Output>,
}

impl Drain for LogDrain {
    fn flush(&mut self) -> Result<(), Error> {
        while let Some(mut log_buffer) = self.log_filled.pop() {
            let _ = self.output.write(&log_buffer);

            // shrink oversized buffer
            if log_buffer.len() > self.buffer_size {
                log_buffer.truncate(self.buffer_size);
                log_buffer.shrink_to_fit();
                log_buffer.clear();
            }

            // recycle the buffer, buffer will be dropped if the pool is full
            log_buffer.clear();
            let _ = self.log_cleared.push(log_buffer);
        }
        self.output.flush()
    }
}

/// A type to construct a basic `AsyncLog` which routes all log messages to a
/// single `Output`.
pub struct LogBuilder {
    total_buffer_size: usize,
    log_message_size: usize,
    format: FormatFunction,
    level_filter: LevelFilter,
    output: Option<Box<dyn Output>>,
}

impl Default for LogBuilder {
    fn default() -> Self {
        Self {
            total_buffer_size: 4 * 1024 * 1024,
            log_message_size: 1024,
            format: default_format,
            level_filter: LevelFilter::Trace,
            output: None,
        }
    }
}

impl LogBuilder {
    /// Create a new log builder.
    pub fn new() -> Self {
        Default::default()
    }

    /// Sets the total buffer size for log messages.
    pub fn total_buffer_size(mut self, bytes: usize) -> Self {
        self.total_buffer_size = bytes;
        self
    }

    /// Sets the log message buffer size. Oversized messages will result in an
    /// extra allocation, but keeping this small allows deeper queues for the
    /// same total buffer size without dropping log messages.
    pub fn log_message_size(mut self, bytes: usize) -> Self {
        self.log_message_size = bytes;
        self
    }

    /// Sets the output for the logger.
    pub fn output(mut self, output: Box<dyn Output>) -> Self {
        self.output = Some(output);
        self
    }

    /// Sets the format function to be used to format messages to this log.
    pub fn format(mut self, format: FormatFunction) -> Self {
        self.format = format;
        self
    }

    /// Consumes the builder and returns a configured `Logger` and `LogHandle`.
    pub(crate) fn build_raw(self) -> Result<(Logger, LogDrain), &'static str> {
        if let Some(output) = self.output {
            let queue_capacity = self.total_buffer_size / self.log_message_size;
            let log_filled = Queue::with_capacity(queue_capacity);
            let log_cleared = Queue::with_capacity(queue_capacity);
            for _ in 0..queue_capacity {
                let _ = log_cleared.push(Vec::with_capacity(self.log_message_size));
            }
            let logger = Logger {
                log_filled: log_filled.clone(),
                log_cleared: log_cleared.clone(),
                buffer_size: self.log_message_size,
                format: self.format,
                level_filter: self.level_filter,
            };
            let log_handle = LogDrain {
                log_filled,
                log_cleared,
                buffer_size: self.log_message_size,
                output,
            };
            Ok((logger, log_handle))
        } else {
            Err("no output configured")
        }
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
