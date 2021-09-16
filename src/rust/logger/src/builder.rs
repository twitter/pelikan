// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::*;

/// A `LogBuilder` allows for configuring a `LogSender` and `LogReceiver` for
/// logging via the usual `log` macros as well as with the `klog!()` command
/// logging macro.
#[derive(Default)]
pub struct LogBuilder {
    debug: DebugConfig,
    command: KlogConfig,
}

impl LogBuilder {
    /// Provide a `DebugConfig` to the log builder which configures the behavior
    /// of the standard `log` macros, including log level, and whether or not to
    /// log to standard out or a file. See the documentation for `DebugConfig`
    /// for more details.
    pub fn debug(mut self, config: DebugConfig) -> Self {
        self.debug = config;
        self
    }

    /// Provide a `KlogConfig` to the log builder which configures the behavior
    /// of the `klog!()` macro for logging commands. This configuration enables
    /// the command log, sets a sampling rate, and specifies a file path for
    /// logging. See the documentation for `KlogConfig` for more details.
    pub fn command(mut self, config: KlogConfig) -> Self {
        self.command = config;
        self
    }

    /// Consumes the builder and returns a `LogSender` and `LogReceiver` which
    /// can then be used for logging.
    pub fn build(self) -> (LogSender, LogReceiver) {
        let (debug_send, debug_recv) = FileLogBuilder::default()
            .msg_size(DEFAULT_MSG_SIZE)
            .buf_size(self.debug.log_nbuf())
            .level(self.debug.log_level())
            .format(default_format)
            .active_path(
                self.debug
                    .log_file()
                    .as_ref()
                    .map(|f| Path::new(f).to_owned()),
            )
            .backup_path(
                self.debug
                    .log_backup()
                    .as_ref()
                    .map(|f| Path::new(f).to_owned()),
            )
            .max_size(self.debug.log_max_size())
            .build();

        let (klog_send, klog_recv) = if let Some(_file) = self.command.file() {
            let (s, r) = FileLogBuilder::default()
                .msg_size(DEFAULT_MSG_SIZE)
                .buf_size(self.command.nbuf())
                .format(klog_format)
                .active_path(
                    self.command
                        .file()
                        .as_ref()
                        .map(|f| Path::new(f).to_owned()),
                )
                .backup_path(
                    self.command
                        .backup()
                        .as_ref()
                        .map(|f| Path::new(f).to_owned()),
                )
                .max_size(self.command.max_size())
                .build();
            let s: Box<dyn Log> = if self.command.sample() > 1 {
                Box::new(SamplingLogSender {
                    sample: self.command.sample(),
                    current: AtomicUsize::new(self.command.sample()),
                    sender: s,
                })
            } else {
                Box::new(s)
            };
            (s, Some(r))
        } else {
            (Box::new(NopLogSender {}) as Box<dyn Log>, None)
        };

        let sender = LogSender {
            debug: Arc::new(Box::new(debug_send)),
            command: Arc::new(klog_send),
            level: self.debug.log_level(),
        };

        let receiver = LogReceiver {
            debug: debug_recv,
            command: klog_recv,
        };

        (sender, receiver)
    }
}
