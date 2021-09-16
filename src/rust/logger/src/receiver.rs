// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::*;

/// A `LogReceiver` is the receiving end of the log queues. It should be owned
/// by the thread which will be flushing the log messages to their respective
/// outputs. It is expected that the `flush()` function will be periodically
/// called to make room for new log messages.
pub struct LogReceiver {
    pub(crate) debug: FileLogReceiver,
    pub(crate) command: Option<FileLogReceiver>,
}

impl LogReceiver {
    /// Flushes the log messages to their respective outputs. Call this
    /// periodically outside of any critical path. For example, an admin or
    /// separate helper thread.
    pub fn flush(&mut self) {
        self.debug.flush();
        if let Some(command) = &mut self.command {
            command.flush()
        };
    }
}
