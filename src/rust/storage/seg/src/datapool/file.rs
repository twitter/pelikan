// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! A file backed datapool implemented by memory mapping the file. Useful for
//! storage on persistent memory (PMEM) and fast NVMe drives.

use crate::datapool::Datapool;
use memmap2::{MmapMut, MmapOptions};

use std::fs::OpenOptions;
use std::path::Path;

const PAGE_SIZE: usize = 4096;

/// The actual `File` datapool which owns the allocated data.
pub struct File {
    mmap: MmapMut,
    size: usize,
}

impl File {
    /// Create a new `File` datapool at the given path and with the specified 
    /// size (in bytes). If a file already exists at the given path, check it is
    /// the right size and open it. Otherwise, open a new file at the given path 
    ///and with the specified size. Returns an error if could not be created, 
    /// size of file is not the right size (opening), couldn't be extended to 
    /// the requested size (creating), or couldn't be mmap'd.
    pub fn create<T: AsRef<Path>>(
        path: T,
        size: usize,
        prefault: bool,
    ) -> Result<Self, std::io::Error> {
        // check if the file exists and is the right size
        let exists = if let Ok(current_size) = std::fs::metadata(&path).map(|m| m.len()) {
            if current_size != size as u64 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "existing file has wrong size",
                ));
            }
            true
        } else {
            false
        };

        let mmap = if exists {
            let f = OpenOptions::new().read(true).write(true).open(path)?;

            unsafe { MmapOptions::new().populate().map_mut(&f)? }
        } else {
            let f = OpenOptions::new()
                .create_new(true)
                .read(true)
                .write(true)
                .open(path)?;
            f.set_len(size as u64)?;

            let mut mmap = unsafe { MmapOptions::new().populate().map_mut(&f)? };

            if prefault {
                let mut offset = 0;
                while offset < size {
                    mmap[offset] = 0;
                    offset += PAGE_SIZE;
                }
                mmap.flush()?;
            }

            mmap
        };

        Ok(Self { mmap, size })
    }
}

impl Datapool for File {
    fn as_slice(&self) -> &[u8] {
        &self.mmap[..self.size]
    }

    fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.mmap[..self.size]
    }

    fn flush(&self) -> Result<(), std::io::Error> {
        self.mmap.flush()
    }
}
