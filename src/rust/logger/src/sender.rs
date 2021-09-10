// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::*;

#[derive(Clone)]
pub struct LogSender {
    pub(crate) debug: Arc<dyn Log>,
    pub(crate) command: Arc<dyn Log>,
    pub(crate) level: Level,
}

impl LogSender {
    pub fn start(self) {
        let level = self.level;
        log::set_boxed_logger(Box::new(self))
            .map(|()| log::set_max_level(level.to_level_filter()))
            .expect("failed to start logger");
    }
}
impl Log for LogSender {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        if metadata.target() == "klog" {
            self.command.enabled(metadata)
        } else {
            self.debug.enabled(metadata)
        }
    }

    fn log(&self, record: &log::Record<'_>) {
        if record.metadata().target() == "klog" {
            self.command.log(record)
        } else {
            self.debug.log(record)
        }
    }

    fn flush(&self) {}
}
