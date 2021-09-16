// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::*;

/// A `FileLogSender` implements `Log` and can be registered as the main logger.
/// At the logging callsite, messages will be filtered and formatted before
/// being sent over a queue to the corresponding `FileLogReceiver`. To reduce
/// runtime allocations, a pool of buffers is maintained and recycled for
/// re-use.
pub struct FileLogSender {
    // level filter for determining if a log message should be logged
    level_filter: LevelFilter,
    // a queue for submitting log messages to the receiver
    sender: Queue<Vec<u8>>,
    // a queue for receiving log buffers for re-use
    buf_pool: Queue<Vec<u8>>,
    // the size of newly created log buffers
    msg_size: usize,
    // a function used to format log messages
    format: FormatFunction,
}

impl Log for FileLogSender {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() <= self.level_filter
    }

    fn log(&self, record: &log::Record<'_>) {
        // if the log message is filtered by the log level, return early
        if !self.enabled(record.metadata()) {
            return;
        }

        // tries to re-use a buffer from the pool or allocate a new buffer
        let mut buffer = self
            .buf_pool
            .pop()
            .unwrap_or_else(|| Vec::with_capacity(self.msg_size));

        // write the log message into the buffer and send to the receiver
        if (self.format)(&mut buffer, recent_local(), record).is_ok() {
            let _ = self.sender.push(buffer);
        }
    }

    fn flush(&self) {}
}
