// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::*;

/// A `FileLogReceiver` receives log messages over a queue, writes those
/// messages to the configured output, rotates log files (if rotation is
/// configured), and returns cleared buffers to the `FileLogSender` for re-use.
pub struct FileLogReceiver {
    // a queue for receiving log messages from the sender
    pub(crate) receiver: Queue<Vec<u8>>,
    // a queue for returning log buffers to the sender for re-use
    pub(crate) buf_pool: Queue<Vec<u8>>,
    // log buffers above this size will not be re-used
    pub(crate) msg_size: usize,
    // current log file path. None implies logging to standard out
    pub(crate) active_path: Option<PathBuf>,
    // backup log file path. None implies the default ".old" extension will be
    // appended to the log file path on rotation
    pub(crate) backup_path: Option<PathBuf>,
    // a buffered writer for writing messages to the log file or standard out
    pub(crate) writer: BufWriter<Box<dyn Write + Send>>,
    // the maximum size of the log file before rotation. 0 implies that there is
    // no size limit.
    pub(crate) max_size: u64,
}

impl FileLogReceiver {
    /// Flush should be periodically called to write log messages to the output
    /// and rotate the log file if it crosses the size threshold.
    pub fn flush(&mut self) {
        while let Some(mut msg) = self.receiver.pop() {
            let _ = self.writer.write(&msg);

            // shrink oversized buffer
            if msg.len() > self.msg_size {
                msg.truncate(self.msg_size);
                msg.shrink_to_fit();
                msg.clear();
            }

            // recycle the buffer, buffer will be dropped if the pool is full
            msg.clear();
            let _ = self.buf_pool.push(msg);
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
