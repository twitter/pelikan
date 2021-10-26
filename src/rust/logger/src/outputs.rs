// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::*;

use std::io::{BufWriter, Error, Write};
use std::path::{Path, PathBuf};

/// An output that writes to `stdout`.
pub struct Stdout {
    writer: BufWriter<std::io::Stdout>,
}

impl Default for Stdout {
    fn default() -> Self {
        Self::new()
    }
}

impl Stdout {
    pub fn new() -> Self {
        Self {
            writer: BufWriter::new(std::io::stdout()),
        }
    }
}

impl Write for Stdout {
    fn write(&mut self, buf: &[u8]) -> Result<usize, Error> {
        self.writer.write(buf)
    }
    fn flush(&mut self) -> std::result::Result<(), Error> {
        self.writer.flush()
    }
}

impl Output for Stdout {}

/// An output that writes to `stderr`.
pub struct Stderr {
    writer: BufWriter<std::io::Stderr>,
}

impl Default for Stderr {
    fn default() -> Self {
        Self::new()
    }
}

impl Stderr {
    pub fn new() -> Self {
        Self {
            writer: BufWriter::new(std::io::stderr()),
        }
    }
}

impl Write for Stderr {
    fn write(&mut self, buf: &[u8]) -> Result<usize, std::io::Error> {
        self.writer.write(buf)
    }
    fn flush(&mut self) -> std::result::Result<(), std::io::Error> {
        self.writer.flush()
    }
}

impl Output for Stderr {}

/// A file based output which allows rotating the current log file off to a
/// backup location.
pub struct File {
    active: PathBuf,
    backup: PathBuf,
    max_size: u64,
    writer: BufWriter<std::fs::File>,
}

impl File {
    /// Create a new file based output. The active path will be the live log
    /// file. When the size of the live log is exceeded, it will automatically
    /// be rotated to the backup path.
    pub fn new<T: AsRef<Path>>(active: T, backup: T, max_size: u64) -> Result<Self, Error> {
        LOG_OPEN.increment();
        let file = match std::fs::File::create(active.as_ref()) {
            Ok(f) => f,
            Err(e) => {
                LOG_OPEN_EX.increment();
                return Err(e);
            }
        };
        let writer = BufWriter::new(file);
        Ok(Self {
            active: active.as_ref().to_owned(),
            backup: backup.as_ref().to_owned(),
            max_size,
            writer,
        })
    }

    /// Return the current size of the live log in bytes.
    fn size(&self) -> Result<u64, Error> {
        Ok(self.writer.get_ref().metadata()?.len())
    }

    /// Rotate the current log file if necessary.
    fn rotate(&mut self) -> Result<(), Error> {
        let size = self.size()?;
        if size >= self.max_size {
            // rename the open file
            std::fs::rename(&self.active, &self.backup)?;

            // create a new file for the live log
            LOG_OPEN.increment();
            let file = match std::fs::File::create(&self.active) {
                Ok(f) => f,
                Err(e) => {
                    LOG_OPEN_EX.increment();
                    return Err(e);
                }
            };
            self.writer = BufWriter::new(file);
        }

        Ok(())
    }
}

impl Write for File {
    fn write(&mut self, buf: &[u8]) -> Result<usize, Error> {
        self.writer.write(buf)
    }
    fn flush(&mut self) -> std::result::Result<(), Error> {
        self.writer.flush()?;
        self.rotate()
    }
}

impl Output for File {}
