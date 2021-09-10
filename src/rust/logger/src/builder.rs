// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::*;

#[derive(Default)]
pub struct LogBuilder {
    debug: DebugConfig,
    command: KlogConfig,
}

impl LogBuilder {
    pub fn debug(mut self, config: DebugConfig) -> Self {
        self.debug = config;
        self
    }

    pub fn command(mut self, config: KlogConfig) -> Self {
        self.command = config;
        self
    }

    pub fn build(self) -> (LogSender, LogReceiver) {
        let (debug_send, debug_recv) = FileLogBuilder::default()
            .buf_size(DEFAULT_MSG_SIZE)
            .buf_pool(DEFAULT_BUFFER_SIZE / DEFAULT_MSG_SIZE)
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
                .buf_size(DEFAULT_MSG_SIZE)
                .buf_pool(DEFAULT_BUFFER_SIZE / DEFAULT_MSG_SIZE)
                .format(default_format)
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
