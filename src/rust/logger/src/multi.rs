// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::*;
use std::io::Error;

use ahash::AHashMap as HashMap;

pub struct MultiLogger {
    default: Option<Box<dyn Log>>,
    targets: HashMap<String, Box<dyn Log>>,
    level_filter: LevelFilter,
}

impl MultiLogger {
    fn get_target(&self, target: &str) -> Option<&dyn Log> {
        self.targets.get(target).map(|t| t.as_ref()).or_else(|| self.default.as_deref())
    }
}

impl Log for MultiLogger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        if metadata.level() > self.level_filter {
            false
        } else if let Some(target) = self.get_target(metadata.target()) {
            target.enabled(metadata)
        } else {
            false
        }
    }

    fn log(&self, record: &log::Record<'_>) {
        if record.metadata().level() > self.level_filter {
            return;
        }
        if let Some(target) = self.get_target(record.target()) {
            if target.enabled(record.metadata()) {
                target.log(record)
            }
        }
    }

    fn flush(&self) {}
}

impl LogEx for MultiLogger {
    fn level_filter(&self) -> LevelFilter {
        self.level_filter
    }
}

pub struct MultiLogDrain {
    default: Option<Box<dyn Drain>>,
    targets: HashMap<String, Box<dyn Drain>>,
}

impl Drain for MultiLogDrain {
    fn flush(&mut self) -> Result<(), Error> {
        if let Some(ref mut default) = self.default {
            default.flush()?;
        }
        for (_target, log_handle) in self.targets.iter_mut() {
            log_handle.flush()?;
        }
        Ok(())
    }
}

pub struct MultiLogBuilder {
    default: Option<AsyncLog>,
    targets: HashMap<String, AsyncLog>,
    level_filter: LevelFilter,
}

impl Default for MultiLogBuilder {
    fn default() -> Self {
        Self {
            default: None,
            targets: HashMap::new(),
            level_filter: LevelFilter::Trace,
        }
    }
}

impl MultiLogBuilder {
    /// Create a new MultiLog builder
    pub fn new() -> Self {
        Default::default()
    }

    pub fn default(mut self, log: AsyncLog) -> Self {
        self.default = Some(log);
        self
    }

    pub fn add_target(mut self, target: &str, log: AsyncLog) -> Self {
        self.targets.insert(target.to_owned(), log);
        self
    }

    pub fn level_filter(mut self, level_filter: LevelFilter) -> Self {
        self.level_filter = level_filter;
        self
    }

    pub fn build(mut self) -> (MultiLogger, MultiLogDrain) {
        let mut loggers = MultiLogger {
            default: None,
            targets: HashMap::new(),
            level_filter: self.level_filter,
        };

        let mut drains = MultiLogDrain {
            default: None,
            targets: HashMap::new(),
        };

        if let Some(log) = self.default.take() {
            loggers.default = Some(log.logger);
            drains.default = Some(log.drain);
        }

        for (name, log) in self.targets.drain() {
            loggers.targets.insert(name.to_owned(), log.logger);
            drains
                .targets
                .insert(name.to_owned(), log.drain);
        }

        (loggers, drains)
    }
}
