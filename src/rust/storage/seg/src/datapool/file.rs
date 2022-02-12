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
    /// If there is a file at the given path, open the `File`.
    /// Otherwise, create a new `File` datapool at the given path and with the specified
    /// size (in bytes). Returns an error if could not
    /// be created, size of file isn't as expected (opening),
    /// couldn't be extended to the requested size (creating), or couldn't be
    /// mmap'd
    pub fn create<T: AsRef<Path>>(
        path: T,
        size: usize,
        prefault: bool,
    ) -> Result<Self, std::io::Error> {
        let metadata = std::fs::metadata(&path);
        let file_exists = metadata.is_ok();
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(path)?;

        // if file exists, check that the size it is expected to have
        // matches its actual size
        if file_exists {
            assert_eq!(metadata?.len() as usize, size);
        } else {
            file.set_len(size as u64)?;
        }

        let mut mmap = unsafe { MmapOptions::new().populate().map_mut(&file)? };

        if !file_exists && prefault {
            let mut offset = 0;
            while offset < size {
                mmap[offset] = 0;
                offset += PAGE_SIZE;
            }
            mmap.flush()?;
        }

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
