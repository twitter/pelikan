// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use rustcommon_time::{recent_local, DateTime, Local, SecondsFormat};
use core::sync::atomic::{AtomicUsize, Ordering};
use std::path::Path;

use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::Arc;

use config::{DebugConfig, KlogConfig};
use mpmc::Queue;

pub use log::*;

const KB: usize = 1024;
const MB: usize = 1024 * KB;

const DEFAULT_MSG_SIZE: usize = KB;
const DEFAULT_BUFFER_SIZE: usize = 2 * MB;

#[macro_export]
macro_rules! klog {
    ($($arg:tt)*) => (
        error!(target: "klog", $($arg)*);
    )
}

pub struct NopSender {}

pub struct LogSender {
    level: LevelFilter,
    sender: Queue<Vec<u8>>,
    buf_pool: Queue<Vec<u8>>,
    buf_size: usize,
    format: FormatFunction,
}

pub struct SamplingLogSender {
    sender: LogSender,
    current: AtomicUsize,
    sample: usize,
}

pub type FormatFunction = fn(
    write: &mut dyn std::io::Write,
    now: DateTime<Local>,
    record: &Record,
) -> Result<(), std::io::Error>;

pub fn default_format(
    w: &mut dyn std::io::Write,
    now: DateTime<Local>,
    record: &Record,
) -> Result<(), std::io::Error> {
    writeln!(
        w,
        "{} {} [{}] {}",
        now.to_rfc3339_opts(SecondsFormat::Secs, true),
        record.level(),
        record.module_path().unwrap_or("<unnamed>"),
        record.args()
    )
}

pub struct LogReceiver {
    receiver: Queue<Vec<u8>>,
    buf_pool: Queue<Vec<u8>>,
    buf_size: usize,
    active_path: Option<PathBuf>,
    backup_path: Option<PathBuf>,
    writer: BufWriter<Box<dyn Write + Send>>,
    max_size: u64,
}

impl LogReceiver {
    pub fn flush(&mut self) {
        while let Some(mut msg) = self.receiver.pop() {
            let _ = self.writer.write(&msg);
            if msg.capacity() <= self.buf_size {
                msg.clear();
                let _ = self.buf_pool.push(msg);
            }
        }
        let _ = self.writer.flush();
        self.rotate();
    }

    pub fn rotate(&mut self) {
        // don't rotate the log if max_size is zero
        if self.max_size == 0 {
            return;
        }

        if let Some(active_path) = &self.active_path {
            if let Ok(Ok(metadata)) = std::fs::File::open(active_path).map(|fd| fd.metadata()) {
                if metadata.len() >= self.max_size {
                    self.writer = BufWriter::new(Box::new(std::io::stdout()));

                    let backup_path = self.backup_path.clone().unwrap_or_else(|| {
                        let mut backup_path = active_path.clone();
                        backup_path.set_extension("old");
                        backup_path
                    });

                    if let Err(e) = std::fs::rename(active_path, &backup_path) {
                        eprintln!(
                            "Failed to rotate log: {:?} -> {:?}",
                            active_path, backup_path
                        );
                        eprintln!("Error: {}", e);
                        panic!("Fatal error");
                    }

                    self.writer = BufWriter::with_capacity(
                        DEFAULT_BUFFER_SIZE,
                        Box::new(
                            std::fs::File::create(active_path).expect("Failed to open log file"),
                        ),
                    );
                }
            } else {
                eprintln!("Failed to read log metadata: {:?}", active_path);
            }
        }
    }
}

pub struct LogBuilder {
    buf_size: usize,
    buf_pool: usize,
    level: LevelFilter,
    format: FormatFunction,
    active_path: Option<PathBuf>,
    backup_path: Option<PathBuf>,
    max_size: u64,
}

impl Default for LogBuilder {
    fn default() -> Self {
        Self {
            buf_size: DEFAULT_MSG_SIZE,
            buf_pool: DEFAULT_BUFFER_SIZE / DEFAULT_MSG_SIZE,
            level: LevelFilter::Info,
            format: default_format,
            active_path: None,
            backup_path: None,
            max_size: 0,
        }
    }
}

impl LogBuilder {
    pub fn buf_size(mut self, size: usize) -> Self {
        self.buf_size = size;
        self
    }

    pub fn buf_pool(mut self, count: usize) -> Self {
        self.buf_pool = count;
        self
    }

    pub fn level(mut self, level: LevelFilter) -> Self {
        self.level = level;
        self
    }

    pub fn format(mut self, format: FormatFunction) -> Self {
        self.format = format;
        self
    }

    pub fn max_size(mut self, bytes: u64) -> Self {
        self.max_size = bytes;
        self
    }

    pub fn active_path(mut self, path: Option<PathBuf>) -> Self {
        self.active_path = path;
        self
    }

    pub fn backup_path(mut self, path: Option<PathBuf>) -> Self {
        self.backup_path = path;
        self
    }

    pub fn build(self) -> (LogSender, LogReceiver) {
        let log_queue = Queue::with_capacity(DEFAULT_BUFFER_SIZE / DEFAULT_MSG_SIZE);
        let buf_queue = Queue::with_capacity(self.buf_pool);

        let sender = LogSender {
            level: self.level,
            format: self.format,
            sender: log_queue.clone(),
            buf_pool: buf_queue.clone(),
            buf_size: self.buf_size,
        };

        let fd: Box<dyn Write + Send> = if let Some(ref path) = self.active_path {
            Box::new(std::fs::File::create(path).expect("Failed to open log file"))
        } else {
            Box::new(std::io::stdout())
        };

        let writer = BufWriter::with_capacity(DEFAULT_BUFFER_SIZE, fd);

        let receiver = LogReceiver {
            receiver: log_queue,
            buf_pool: buf_queue,
            buf_size: self.buf_size,
            writer,
            active_path: self.active_path,
            backup_path: self.backup_path,
            max_size: self.max_size,
        };

        (sender, receiver)
    }
}

impl Log for NopSender {
    fn enabled(&self, _: &log::Metadata<'_>) -> bool {
        false
    }
    fn log(&self, _: &log::Record<'_>) {}
    fn flush(&self) {}
}

impl Log for LogSender {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() <= self.level
    }

    fn log(&self, record: &log::Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let mut buffer = self
            .buf_pool
            .pop()
            .unwrap_or_else(|| Vec::with_capacity(self.buf_size));

        if (self.format)(&mut buffer, recent_local(), record).is_ok() {
            let _ = self.sender.push(buffer);
        }
    }

    fn flush(&self) {}
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

#[derive(Clone)]
pub struct PelikanLogSender {
    debug: Arc<dyn Log>,
    command: Arc<dyn Log>,
    level: Level,
}

impl PelikanLogSender {
    pub fn start(self) {
        let level = self.level;
        log::set_boxed_logger(Box::new(self))
            .map(|()| log::set_max_level(level.to_level_filter()))
            .expect("failed to start logger");
    }
}

pub struct PelikanLogReceiver {
    debug: LogReceiver,
    command: Option<LogReceiver>,
}

impl PelikanLogReceiver {
    pub fn flush(&mut self) {
        self.debug.flush();
        if let Some(command) = &mut self.command {
            command.flush()
        };
    }
}

#[derive(Default)]
pub struct PelikanLogBuilder {
    debug: DebugConfig,
    command: KlogConfig,
}

impl PelikanLogBuilder {
    pub fn debug(mut self, config: DebugConfig) -> Self {
        self.debug = config;
        self
    }

    pub fn command(mut self, config: KlogConfig) -> Self {
        self.command = config;
        self
    }

    pub fn build(self) -> (PelikanLogSender, PelikanLogReceiver) {
        let (debug_send, debug_recv) = LogBuilder::default()
            .buf_size(DEFAULT_MSG_SIZE)
            .buf_pool(DEFAULT_BUFFER_SIZE / DEFAULT_MSG_SIZE)
            .level(filter_for_level(self.debug.log_level()))
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
            let (s, r) = LogBuilder::default()
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
            (Box::new(NopSender {}) as Box<dyn Log>, None)
        };

        let sender = PelikanLogSender {
            debug: Arc::new(Box::new(debug_send)),
            command: Arc::new(klog_send),
            level: self.debug.log_level(),
        };

        let receiver = PelikanLogReceiver {
            debug: debug_recv,
            command: klog_recv,
        };

        (sender, receiver)
    }
}

fn filter_for_level(level: Level) -> LevelFilter {
    match level {
        Level::Trace => LevelFilter::Trace,
        Level::Debug => LevelFilter::Debug,
        Level::Info => LevelFilter::Info,
        Level::Warn => LevelFilter::Warn,
        Level::Error => LevelFilter::Error,
    }
}

impl Log for PelikanLogSender {
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
