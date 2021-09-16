// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::*;

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

        // allocate all the msg buffers
        for _ in 0..msg_count {
            let _ = buf_queue.push(vec![0; self.msg_size]);
        }

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

