// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! A logging backend which will write to standard output or to a log file with
//! configurable log rotation. This logging backend re-uses a pool of buffers
//! for sending log messages from the sender (at the log call site) to the
//! receiver. The receiver should be periodically flushed outside of the
//! critical path.

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

/// A `FileLogReceiver` receives log messages over a queue, writes those
/// messages to the configured output, rotates log files (if rotation is
/// configured), and returns cleared buffers to the `FileLogSender` for re-use.
pub struct FileLogReceiver {
    // a queue for receiving log messages from the sender
    receiver: Queue<Vec<u8>>,
    // a queue for returning log buffers to the sender for re-use
    buf_pool: Queue<Vec<u8>>,
    // log buffers above this size will not be re-used
    msg_size: usize,
    // current log file path. None implies logging to standard out
    active_path: Option<PathBuf>,
    // backup log file path. None implies the default ".old" extension will be
    // appended to the log file path on rotation
    backup_path: Option<PathBuf>,
    // a buffered writer for writing messages to the log file or standard out
    writer: BufWriter<Box<dyn Write + Send>>,
    // the maximum size of the log file before rotation. 0 implies that there is
    // no size limit.
    max_size: u64,
}

impl FileLogReceiver {
    /// Flush should be periodically called to write log messages to the output
    /// and rotate the log file if it crosses the size threshold.
    pub fn flush(&mut self) {
        while let Some(mut msg) = self.receiver.pop() {
            let _ = self.writer.write(&msg);
            // recycle the buffer if it's not oversized
            if msg.capacity() <= self.msg_size {
                msg.clear();
                let _ = self.buf_pool.push(msg);
            }
        }
        // since we are using a buffered writer, we need to flush it
        let _ = self.writer.flush();
        // trigger log rotation
        self.rotate();
    }

    /// Internal function to conditionally rotate the log file.
    fn rotate(&mut self) {
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

/// A structure for configuring a file logger.
pub struct FileLogBuilder {
    // default log message size in bytes
    msg_size: usize,
    // total buffer size in bytes
    buf_size: usize,
    // log level filter
    level_filter: LevelFilter,
    // log message formatting function
    format: FormatFunction,
    // current log file, None implies standard output
    active_path: Option<PathBuf>,
    // backup log file, None will default to ".old" extension being appended
    backup_path: Option<PathBuf>,
    // maximum log file size, when the log grows above this size, rotation will
    // occur
    max_size: u64,
}

impl Default for FileLogBuilder {
    fn default() -> Self {
        Self {
            msg_size: DEFAULT_MSG_SIZE,
            buf_size: DEFAULT_BUFFER_SIZE,
            level_filter: LevelFilter::Info,
            format: default_format,
            active_path: None,
            backup_path: None,
            max_size: 0,
        }
    }
}

impl FileLogBuilder {
    /// Sets the default log message buffer size
    pub fn msg_size(mut self, bytes: usize) -> Self {
        self.msg_size = bytes;
        self
    }

    /// Set the total size of the log buffer pool
    pub fn buf_size(mut self, bytes: usize) -> Self {
        self.buf_size = bytes;
        self
    }

    /// Set the logging level
    pub fn level(mut self, level: Level) -> Self {
        self.level_filter = level.to_level_filter();
        self
    }

    /// Set the formatting function to use for log messages
    pub fn format(mut self, format: FormatFunction) -> Self {
        self.format = format;
        self
    }

    /// Set the maximum file size before rotation. Note that this size may be
    /// exceeded by the size of the log buffer pool.
    pub fn max_size(mut self, bytes: u64) -> Self {
        self.max_size = bytes;
        self
    }

    /// Specify a file path for logging. If no path is provided, then log
    /// messages will be written to standard out.
    pub fn active_path(mut self, path: Option<PathBuf>) -> Self {
        self.active_path = path;
        self
    }

    /// Specify a file path to backup the log file on rotation. If no path is
    /// provided, then the ".old" extension will be added to the active path.
    pub fn backup_path(mut self, path: Option<PathBuf>) -> Self {
        self.backup_path = path;
        self
    }

    /// Consume the builder and return a configured `FileLogSender` and
    /// `FileLogReceiver`.
    pub fn build(self) -> (FileLogSender, FileLogReceiver) {
        let msg_count = self.buf_size / self.msg_size;
        let log_queue = Queue::with_capacity(msg_count);
        let buf_queue = Queue::with_capacity(msg_count);

        let sender = FileLogSender {
            level_filter: self.level_filter,
            format: self.format,
            sender: log_queue.clone(),
            buf_pool: buf_queue.clone(),
            msg_size: self.msg_size,
        };

        let fd: Box<dyn Write + Send> = if let Some(ref path) = self.active_path {
            Box::new(std::fs::File::create(path).expect("Failed to open log file"))
        } else {
            Box::new(std::io::stdout())
        };

        let writer = BufWriter::with_capacity(DEFAULT_BUFFER_SIZE, fd);

        let receiver = FileLogReceiver {
            receiver: log_queue,
            buf_pool: buf_queue,
            msg_size: self.msg_size,
            writer,
            active_path: self.active_path,
            backup_path: self.backup_path,
            max_size: self.max_size,
        };

        (sender, receiver)
    }
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
