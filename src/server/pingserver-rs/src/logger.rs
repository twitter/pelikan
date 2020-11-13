// Copyright 2019 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

#![deny(clippy::all)]

use chrono::prelude::*;

#[macro_export]
macro_rules! fatal {
    () => (
        error!();
        std::process::exit(1);
        );
    ($fmt:expr) => (
        error!($fmt);
        std::process::exit(1);
        );
    ($fmt:expr, $($arg:tt)*) => (
        error!($fmt, $($arg)*);
        std::process::exit(1);
        );
}

pub use log::{Level, LevelFilter, Metadata, Record, SetLoggerError};

pub struct Logger {
    label: Option<&'static str>,
    level: Level,
}

impl Logger {
    pub fn new() -> Self {
        Logger {
            label: None,
            level: Level::Info,
        }
    }

    pub fn init(self) -> Result<(), SetLoggerError> {
        println!("log level: {:?}", self.level);
        let level = self.level;
        log::set_boxed_logger(Box::new(self)).map(|()| log::set_max_level(level.to_level_filter()))
    }

    pub fn label(mut self, label: &'static str) -> Self {
        self.label = Some(label);
        self
    }

    pub fn level(mut self, level: Level) -> Self {
        self.level = level;
        self
    }
}

impl Default for Logger {
    fn default() -> Self {
        Self::new()
    }
}

impl log::Log for Logger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.level
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let target = if let Some(label) = self.label {
                match log::max_level() {
                    LevelFilter::Debug | LevelFilter::Trace => {
                        format!("{}::{}", label, record.target())
                    }
                    _ => label.to_string(),
                }
            } else {
                record.target().to_string()
            };
            println!(
                "{} {:<5} [{}] {}",
                Utc::now(),
                record.level(),
                target,
                record.args()
            );
        }
    }

    fn flush(&self) {}
}
