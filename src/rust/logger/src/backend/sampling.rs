// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::*;

pub struct SamplingLogSender {
    pub(crate) sender: FileLogSender,
    pub(crate) current: AtomicUsize,
    pub(crate) sample: usize,
}

impl Log for SamplingLogSender {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        self.sender.enabled(metadata)
    }

    fn log(&self, record: &log::Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }
        if self.current.fetch_add(1, Ordering::Relaxed) == self.sample {
            self.current.fetch_sub(self.sample, Ordering::Relaxed);
            self.sender.log(record)
        }
    }

    fn flush(&self) {}
}
