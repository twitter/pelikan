// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::*;

pub struct FileLogSender {
    level_filter: LevelFilter,
    sender: Queue<Vec<u8>>,
    buf_pool: Queue<Vec<u8>>,
    buf_size: usize,
    format: FormatFunction,
}

pub struct FileLogReceiver {
    receiver: Queue<Vec<u8>>,
    buf_pool: Queue<Vec<u8>>,
    buf_size: usize,
    active_path: Option<PathBuf>,
    backup_path: Option<PathBuf>,
    writer: BufWriter<Box<dyn Write + Send>>,
    max_size: u64,
}

impl FileLogReceiver {
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

pub struct FileLogBuilder {
    buf_size: usize,
    buf_pool: usize,
    level_filter: LevelFilter,
    format: FormatFunction,
    active_path: Option<PathBuf>,
    backup_path: Option<PathBuf>,
    max_size: u64,
}

impl Default for FileLogBuilder {
    fn default() -> Self {
        Self {
            buf_size: DEFAULT_MSG_SIZE,
            buf_pool: DEFAULT_BUFFER_SIZE / DEFAULT_MSG_SIZE,
            level_filter: LevelFilter::Info,
            format: default_format,
            active_path: None,
            backup_path: None,
            max_size: 0,
        }
    }
}

impl FileLogBuilder {
    pub fn buf_size(mut self, size: usize) -> Self {
        self.buf_size = size;
        self
    }

    pub fn buf_pool(mut self, count: usize) -> Self {
        self.buf_pool = count;
        self
    }

    pub fn level(mut self, level: Level) -> Self {
        self.level_filter = level.to_level_filter();
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

    pub fn build(self) -> (FileLogSender, FileLogReceiver) {
        let log_queue = Queue::with_capacity(DEFAULT_BUFFER_SIZE / DEFAULT_MSG_SIZE);
        let buf_queue = Queue::with_capacity(self.buf_pool);

        let sender = FileLogSender {
            level_filter: self.level_filter,
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

        let receiver = FileLogReceiver {
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

impl Log for FileLogSender {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() <= self.level_filter
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
