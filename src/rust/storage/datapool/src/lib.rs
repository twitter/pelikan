// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use blake3::Hash;
use rustcommon_time::{Instant, Seconds, Nanoseconds, UnixInstant};
use core::ops::Range;
use std::fs::{File, OpenOptions};
use std::io::{Error, ErrorKind, Read, Seek, SeekFrom, Write};
use std::path::Path;

#[cfg(os = "linux")]
use std::os::unix::fs::OpenOptionsExt;

use memmap2::{MmapMut, MmapOptions};

const PAGE_SIZE: usize = 4096;
const HEADER_SIZE: usize = core::mem::size_of::<Header>();
const MAGIC: [u8; 8] = *b"PELIKAN!";

// NOTE: this must be incremented if there are breaking changes to the on-disk
// format
const VERSION: u64 = 0;

/// The datapool trait defines the abstraction that each datapool implementation
/// should conform to.
pub trait Datapool: Send {
    /// Immutable borrow of the data within the datapool
    fn as_slice(&self) -> &[u8];

    /// Mutable borrow of the data within the datapool
    fn as_mut_slice(&mut self) -> &mut [u8];

    /// Performs any actions necessary to persist the data to the backing store.
    /// This may be a no-op for datapools which cannot persist data.
    fn flush(&mut self) -> Result<(), std::io::Error>;

    fn len(&self) -> usize {
        self.as_slice().len()
    }
}

/// Represents volatile in-memory storage.
pub struct Memory {
    mmap: MmapMut,
    size: usize,
}

impl Memory {
    pub fn create(size: usize) -> Result<Self, std::io::Error> {
        // mmap an anonymous region
        let mut mmap = MmapOptions::new().populate().len(size).map_anon()?;

        // causes the mmap'd region to be prefaulted by writing a zero at the
        // start of each page
        let mut offset = 0;
        while offset < size {
            mmap[offset] = 0;
            offset += PAGE_SIZE;
        }

        Ok(Self { mmap, size })
    }
}

impl Datapool for Memory {
    fn as_slice(&self) -> &[u8] {
        &self.mmap[..self.size]
    }

    fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.mmap[..self.size]
    }

    fn flush(&mut self) -> Result<(), std::io::Error> {
        self.mmap.flush()
    }
}

// NOTE: make sure this is a whole number of pages and that all fields which are
// accessed are properly aligned to avoid undefined behavior.
#[repr(packed)]
pub struct Header {
    checksum: [u8; 32],
    magic: [u8; 8],
    version: u64,
    time_monotonic_s: Instant<Seconds<u32>>,
    time_unix_s: UnixInstant<Seconds<u32>>,
    time_monotonic_ns: Instant<Nanoseconds<u64>>,
    time_unix_ns: UnixInstant<Nanoseconds<u64>>,
    user_version: u64,
    _pad: [u8; 4016],
}

impl Header {
    fn new() -> Self {
        Self {
            checksum: [0; 32],
            magic: MAGIC,
            version: VERSION,
            time_monotonic_s: Instant::<Seconds<u32>>::now(),
            time_unix_s: UnixInstant::<Seconds<u32>>::now(),
            time_monotonic_ns: Instant::<Nanoseconds<u64>>::now(),
            time_unix_ns: UnixInstant::<Nanoseconds<u64>>::now(),
            user_version: 0,
            _pad: [0; 4016],
        }
    }

    fn as_bytes(&self) -> &[u8] {
        unsafe {
            std::slice::from_raw_parts((&*self as *const Header) as *const u8, HEADER_SIZE)
        }
    }

    fn checksum(&self) -> &[u8; 32] {
        &self.checksum
    }

    fn set_checksum(&mut self, hash: Hash) {
        for (idx, byte) in hash.as_bytes()[0..32].iter().enumerate() {
            self.checksum[idx] = *byte;
        }
    }

    fn zero_checksum(&mut self) {
        for byte in self.checksum.iter_mut() {
            *byte = 0;
        }
    }

    fn check(&self) -> Result<(), std::io::Error> {
        self.check_magic()?;
        self.check_version()
    }

    fn check_version(&self) -> Result<(), std::io::Error> {
        if self.version != VERSION {
            Err(Error::new(ErrorKind::Other, "file has incompatible version"))
        } else {
            Ok(())
        }
    }

    fn check_magic(&self) -> Result<(), std::io::Error> {
        if self.magic[0..8] == MAGIC[0..8] {
            Ok(())
        } else {
            Err(Error::new(ErrorKind::Other, "header is not recognized"))
        }
    }

    fn user_version(&self) -> u64 {
        self.user_version
    }

    fn set_user_version(&mut self, user_version: u64) {
        self.user_version = user_version;
    }
}

/// Represents storage that primarily exists in a file. This is best used in
/// combination with a DAX-aware filesystem on persistent memory to avoid page
/// cache pollution and interference. It can be used for volatile storage or
/// allow to resume from a clean shutdown.
pub struct MmapFile {
    mmap: MmapMut,
    data: Range<usize>,
    user_version: u64,
}

impl MmapFile {
    /// Open an existing `MmapFile` datapool at the given path and with the
    /// specified size (in bytes). Returns an error if the file does not exist,
    /// does not match the expected size, could not be mmap'd, or is otherwise
    /// determined to be corrupt.
    pub fn open<T: AsRef<Path>>(path: T, data_size: usize, user_version: u64) -> Result<Self, std::io::Error> {
        // we need the data size to be a whole number of pages
        let pages = ((HEADER_SIZE + data_size) as f64 / PAGE_SIZE as f64).ceil() as usize;

        let total_size = pages * PAGE_SIZE;

        // open an existing file for read and write access
        let file = OpenOptions::new()
            .create_new(false)
            .read(true)
            .write(true)
            .open(path)?;

        // make sure the file size matches the expected size
        if file.metadata()?.len() != total_size as u64 {
            return Err(Error::new(ErrorKind::Other, "filesize mismatch"));
        }

        // data resides after a small header
        let data = Range {
            start: HEADER_SIZE,
            end: HEADER_SIZE + data_size,
        };

        // mmap the file
        let mmap = unsafe { MmapOptions::new().populate().map_mut(&file)? };

        // load copy the header from the mmap'd file
        let mut header = [0; HEADER_SIZE];
        header.copy_from_slice(&mmap[0..HEADER_SIZE]);

        // convert the header to a struct so we can check and manipulate it
        let header = unsafe { &mut *(header.as_ptr() as *mut Header) };

        // check the header
        header.check()?;

        // check the user version
        if header.user_version() != user_version {
            return Err(Error::new(ErrorKind::Other, "user version mismatch"));
        }

        // zero out the checksum in the header copy
        header.zero_checksum();

        // create a hasher
        let mut hasher = blake3::Hasher::new();

        // hash the header with a zero'd checksum
        hasher.update(&header.as_bytes());

        // calculates the hash of the data region, as a side effect this
        // prefaults all the pages
        hasher.update(&mmap[data.start..data.end]);

        // finalize the hash
        let hash = hasher.finalize();

        // compare the stored checksum in the file to the calculated checksum
        if mmap[0..32] != hash.as_bytes()[0..32] {
            return Err(Error::new(ErrorKind::Other, "checksum mismatch"));
        }

        // return the loaded datapool
        Ok(Self { mmap, data, user_version })
    }

    /// Create a new `File` datapool at the given path and with the specified
    /// size (in bytes). Returns an error if the file already exists, could not
    /// be created, couldn't be extended to the requested size, or couldn't be
    /// mmap'd.
    pub fn create<T: AsRef<Path>>(path: T, data_size: usize, user_version: u64) -> Result<Self, std::io::Error> {
        // we need the data size to be a whole number of pages
        let pages = ((HEADER_SIZE + data_size) as f64 / PAGE_SIZE as f64).ceil() as usize;

        let total_size = pages * PAGE_SIZE;

        // data resides after a small header
        let data = Range {
            start: HEADER_SIZE,
            end: total_size,
        };

        // create a new file with read and write access
        let file = OpenOptions::new()
            .create_new(true)
            .read(true)
            .write(true)
            .open(path)?;

        // grow the file to match the total size
        file.set_len(total_size as u64)?;

        // mmap the file
        let mut mmap = unsafe { MmapOptions::new().populate().map_mut(&file)? };

        // causes the mmap'd region to be prefaulted by writing a zero at the
        // start of each page
        let mut offset = 0;
        while offset < total_size {
            mmap[offset] = 0;
            offset += PAGE_SIZE;
        }
        mmap.flush()?;

        Ok(Self { mmap, data, user_version })
    }

    pub fn header(&self) -> &Header {
        // load copy the header from the mmap'd file
        let mut header = [0; HEADER_SIZE];
        header.copy_from_slice(&self.mmap[0..HEADER_SIZE]);

        // convert the header to a struct
        unsafe { &*(header.as_ptr() as *const Header) }
    }

    pub fn time_monotonic_s(&self) -> Instant<Seconds<u32>> {
        self.header().time_monotonic_s
    }

    pub fn time_monotonic_ns(&self) -> Instant<Nanoseconds<u64>> {
        self.header().time_monotonic_ns
    }

    pub fn time_unix_s(&self) -> UnixInstant<Seconds<u32>> {
        self.header().time_unix_s
    }

    pub fn time_unix_ns(&self) -> UnixInstant<Nanoseconds<u64>> {
        self.header().time_unix_ns
    }
}

impl Datapool for MmapFile {
    fn as_slice(&self) -> &[u8] {
        &self.mmap[self.data.start..self.data.end]
    }

    fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.mmap[self.data.start..self.data.end]
    }

    fn flush(&mut self) -> Result<(), std::io::Error> {
        // flush everything to the underlying file
        self.mmap.flush()?;

        // initialize the hasher
        let mut hasher = blake3::Hasher::new();

        // prepare the header
        let mut header = Header::new();

        // set the user version
        header.set_user_version(self.user_version);

        // hash the header
        hasher.update(&header.as_bytes());

        // calculate the number of data pages to be copied
        let data_pages = (self.mmap.len() - HEADER_SIZE) / PAGE_SIZE;

        // hash the data region
        for page in 0..data_pages {
            let start = page * PAGE_SIZE + HEADER_SIZE;
            let end = start + PAGE_SIZE;
            hasher.update(&self.mmap[start..end]);
        }

        // finalize the hash
        let hash = hasher.finalize();

        // set the header checksum with the calculated hash
        header.set_checksum(hash);

        // write the header to the file using memcpy
        // SAFETY: we know the source is exactly HEADER_SIZE and that the
        // destination is at least as large. We also know that they are both
        // properly aligned and do not overlap.
        unsafe {
            let src = header.as_bytes().as_ptr();
            let dst = self.mmap.as_mut_ptr();
            std::ptr::copy_nonoverlapping(src, dst, HEADER_SIZE);
        }

        // flush again
        self.mmap.flush()
    }
}

/// Represents storage that is primarily in-memory, but has an associated file
/// which backs it onto more durable storage media. This allows us to use DRAM
/// to provide fast access to the storage region but with the ability to save
/// and restore from some file. It is recommended this file be kept on a fast
/// local disk (eg: NVMe), but it is not strictly required. Unlike simply using
/// mmap on the file, this ensures all the data is kept resident in-memory.
///
/// This currently attempts to use `O_DIRECT` on Linux to avoid the page cache.
/// No attempts are made to avoid similar pollution on other operating systems
/// at this time. Further, there are situations in which even with `O_DIRECT`,
/// the operating system may still buffer access to/from the file. No effort is
/// made to detect, avoid, or handle this situation.
pub struct FileBackedMemory {
    memory: Memory,
    header: Box<[u8]>,
    file: File,
    file_data: Range<usize>,
    user_version: u64,
}

impl FileBackedMemory {
    pub fn open<T: AsRef<Path>>(path: T, data_size: usize, user_version: u64) -> Result<Self, std::io::Error> {
        // we need the data size to be a whole number of pages for direct io
        let pages = ((HEADER_SIZE + data_size) as f64 / PAGE_SIZE as f64).ceil() as usize;

        // total size must be larger than the requested size to allow for the
        // header
        let file_total_size = Range {
            start: 0,
            end: pages * PAGE_SIZE,
        };

        // data resides after a small header
        let file_data = Range {
            start: HEADER_SIZE,
            end: HEADER_SIZE + data_size,
        };

        // create a new file with read and write access
        #[cfg(os = "linux")]
        let mut file = OpenOptions::new()
            .create_new(false)
            .custom_flags(libc::O_DIRECT)
            .read(true)
            .write(true)
            .open(path)?;

        #[cfg(not(os = "linux"))]
        let mut file = OpenOptions::new()
            .create_new(false)
            .read(true)
            .write(true)
            .open(path)?;

        // make sure the file size matches the expected size
        if file.metadata()?.len() != file_total_size.end as u64 {
            return Err(Error::new(ErrorKind::Other, "filesize mismatch"));
        }

        // calculate the page range for the data region
        let data_pages = (file_data.end - file_data.start) / PAGE_SIZE;

        // reserve memory for the data
        let mut memory = Memory::create(data_size)?;

        // seek to start of header
        file.seek(SeekFrom::Start(0))?;

        // prepare the header to read from disk
        let mut header = [0; HEADER_SIZE];

        // read the header from disk
        loop {
            if file.read(&mut header[0..PAGE_SIZE])? == PAGE_SIZE {
                break;
            }
            file.seek(SeekFrom::Start(0))?;
        }

        // create a new hasher to checksum the file content, including the
        // header with a zero'd checksum
        let mut hasher = blake3::Hasher::new();

        // turn the raw header into the struct
        let header = unsafe { &mut *(header.as_ptr() as *mut Header) };

        // check the header
        header.check()?;

        // check the user version
        if header.user_version() != user_version {
            return Err(Error::new(ErrorKind::Other, "user version mismatch"));
        }

        // copy the checksum out of the header and zero it in the header
        let file_checksum = header.checksum().to_owned();
        header.zero_checksum();

        // hash the header with the zero'd checksum
        hasher.update(&header.as_bytes());

        // seek to start of the data
        file.seek(SeekFrom::Start(file_data.start as u64))?;

        // read the data region from the file, copy it into memory and hash it
        // in a single pass
        for page in 0..data_pages {
            // retry the read until a complete page is read
            loop {
                let start = page * PAGE_SIZE;
                let end = start + PAGE_SIZE;

                if file.read(&mut memory.as_mut_slice()[start..end])? == PAGE_SIZE {
                    hasher.update(&memory.as_slice()[start..end]);
                    break;
                }
                // if the read was incomplete, we seek back to the right spot in
                // the file
                file.seek(SeekFrom::Start((HEADER_SIZE + start) as u64))?;
            }
        }

        // finalize the hash
        let hash = hasher.finalize();

        // compare the checksum agaianst what's in the header
        if file_checksum[0..32] != hash.as_bytes()[0..32] {
            return Err(Error::new(ErrorKind::Other, "checksum mismatch"));
        }

        // return the loaded datapool
        Ok(Self {
            memory,
            header: header.as_bytes().to_owned().into_boxed_slice(),
            file,
            file_data,
            user_version,
        })
    }

    pub fn create<T: AsRef<Path>>(path: T, data_size: usize, user_version: u64) -> Result<Self, std::io::Error> {
        // we need the data size to be a whole number of pages for direct io
        let pages = ((HEADER_SIZE + data_size) as f64 / PAGE_SIZE as f64).ceil() as usize;

        // total size must be larger than the requested size to allow for the
        // header
        let file_total_size = Range {
            start: 0,
            end: pages * PAGE_SIZE,
        };

        // data resides after a small header
        let file_data = Range {
            start: HEADER_SIZE,
            end: pages * PAGE_SIZE,
        };

        // create a new file with read and write access
        #[cfg(os = "linux")]
        let mut file = OpenOptions::new()
            .create_new(true)
            .custom_flags(libc::O_DIRECT)
            .read(true)
            .write(true)
            .open(path)?;

        #[cfg(not(os = "linux"))]
        let mut file = OpenOptions::new()
            .create_new(true)
            .read(true)
            .write(true)
            .open(path)?;

        // grow the file to match the total size
        file.set_len(file_total_size.end as u64)?;

        // causes file to be zeroed out
        for page in 0..pages {
            loop {
                if file.write(&[0; PAGE_SIZE])? == PAGE_SIZE {
                    break;
                }
                file.seek(SeekFrom::Start((page * PAGE_SIZE) as u64))?;
            }
        }
        file.sync_all()?;

        let memory = Memory::create(data_size)?;

        Ok(Self {
            memory,
            header: vec![0; HEADER_SIZE].into_boxed_slice(),
            file,
            file_data,
            user_version,
        })
    }

    pub fn header(&self) -> &Header {
        unsafe { &*(self.header.as_ptr() as *const Header) }
    }

    pub fn time_monotonic_s(&self) -> Instant<Seconds<u32>> {
        self.header().time_monotonic_s
    }

    pub fn time_monotonic_ns(&self) -> Instant<Nanoseconds<u64>> {
        self.header().time_monotonic_ns
    }

    pub fn time_unix_s(&self) -> UnixInstant<Seconds<u32>> {
        self.header().time_unix_s
    }

    pub fn time_unix_ns(&self) -> UnixInstant<Nanoseconds<u64>> {
        self.header().time_unix_ns
    }
}

impl Datapool for FileBackedMemory {
    fn as_slice(&self) -> &[u8] {
        self.memory.as_slice()
    }

    fn as_mut_slice(&mut self) -> &mut [u8] {
        self.memory.as_mut_slice()
    }

    fn flush(&mut self) -> Result<(), std::io::Error> {
        // initialize the hasher
        let mut hasher = blake3::Hasher::new();

        // prepare the header
        let mut header = Header::new();

        // set the user version
        header.set_user_version(self.user_version);

        // hash the header with a zero'd checksum
        hasher.update(&header.as_bytes());

        // calculate the number of data pages to be copied
        let data_pages = (self.file_data.end - self.file_data.start) / PAGE_SIZE;

        // write the data region to the file and hash it in one pass
        self.file.seek(SeekFrom::Start(HEADER_SIZE as u64))?;
        for page in 0..data_pages {
            loop {
                let start = page * PAGE_SIZE;
                let end = start + PAGE_SIZE;
                if self.file.write(&self.memory.as_slice()[start..end])? == PAGE_SIZE {
                    hasher.update(&self.memory.as_slice()[start..end]);
                    break;
                }
                self.file
                    .seek(SeekFrom::Start((HEADER_SIZE + start) as u64))?;
            }
        }

        // finalize the hash
        let hash = hasher.finalize();

        // set the checksum in the header to the calculated hash
        header.set_checksum(hash);

        // write the header to the file
        self.file.seek(SeekFrom::Start(0))?;
        loop {
            if self.file.write(&header.as_bytes())? == HEADER_SIZE {
                break;
            }
            self.file.seek(SeekFrom::Start(0))?;
        }

        self.file.sync_all()?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn header_size() {
        // NOTE: make sure this is an even multiple of the page size
        assert_eq!(std::mem::size_of::<Header>(), PAGE_SIZE);
    }

    #[test]
    fn memory_datapool() {
        let datapool = Memory::create(2 * PAGE_SIZE).expect("failed to create pool");
        assert_eq!(datapool.len(), 2 * PAGE_SIZE);
    }

    #[test]
    fn mmapfile_datapool() {
        let tempdir = TempDir::new().expect("failed to generate tempdir");
        let mut path = tempdir.into_path();
        path.push("mmap_test.data");

        let magic_a = [0xDE, 0xCA, 0xFB, 0xAD];
        let magic_b = [0xBA, 0xDC, 0x0F, 0xFE, 0xEB, 0xAD, 0xCA, 0xFE];

        // create a datapool, write some content to it, and close it
        {
            let mut datapool = MmapFile::create(&path, 2 * PAGE_SIZE, 0).expect("failed to create pool");
            assert_eq!(datapool.len(), 2 * PAGE_SIZE);
            datapool.flush().expect("failed to flush");

            for (i, byte) in magic_a.iter().enumerate() {
                datapool.as_mut_slice()[i] = *byte;
            }
            datapool.flush().expect("failed to flush");
        }

        // open the datapool and check the content, then update it
        {
            let mut datapool = MmapFile::open(&path, 2 * PAGE_SIZE, 0).expect("failed to create pool");
            assert_eq!(datapool.len(), 2 * PAGE_SIZE);
            assert_eq!(datapool.as_slice()[0..4], magic_a[0..4]);
            assert_eq!(datapool.as_slice()[4..8], [0; 4]);

            for (i, byte) in magic_b.iter().enumerate() {
                datapool.as_mut_slice()[i] = *byte;
            }
            datapool.flush().expect("failed to flush");
        }

        // open the datapool again, and check that it has the new data
        {
            let datapool = MmapFile::open(&path, 2 * PAGE_SIZE, 0).expect("failed to create pool");
            assert_eq!(datapool.len(), 2 * PAGE_SIZE);
            assert_eq!(datapool.as_slice()[0..8], magic_b[0..8]);
        }

        // check that the datapool does not open if the user version is incorrect
        {
            assert!(MmapFile::open(&path, 2 * PAGE_SIZE, 1).is_err());
        }
    }

    #[test]
    fn filebackedmemory_datapool() {
        let tempdir = TempDir::new().expect("failed to generate tempdir");
        let mut path = tempdir.into_path();
        path.push("mmap_test.data");

        let magic_a = [0xDE, 0xCA, 0xFB, 0xAD];
        let magic_b = [0xBA, 0xDC, 0x0F, 0xFE, 0xEB, 0xAD, 0xCA, 0xFE];

        // create a datapool, write some content to it, and close it
        {
            let mut datapool =
                FileBackedMemory::create(&path, 2 * PAGE_SIZE, 0).expect("failed to create pool");
            assert_eq!(datapool.len(), 2 * PAGE_SIZE);
            datapool.flush().expect("failed to flush");

            for (i, byte) in magic_a.iter().enumerate() {
                datapool.as_mut_slice()[i] = *byte;
            }
            datapool.flush().expect("failed to flush");
        }

        // open the datapool and check the content, then update it
        {
            let mut datapool = FileBackedMemory::open(&path, 2 * PAGE_SIZE, 0).expect("failed to open pool");
            assert_eq!(datapool.len(), 2 * PAGE_SIZE);
            assert_eq!(datapool.as_slice()[0..4], magic_a[0..4]);
            assert_eq!(datapool.as_slice()[4..8], [0; 4]);

            for (i, byte) in magic_b.iter().enumerate() {
                datapool.as_mut_slice()[i] = *byte;
            }
            datapool.flush().expect("failed to flush");
        }

        // open the datapool again, and check that it has the new data
        {
            let datapool = FileBackedMemory::open(&path, 2 * PAGE_SIZE, 0).expect("failed to create pool");
            assert_eq!(datapool.len(), 2 * PAGE_SIZE);
            assert_eq!(datapool.as_slice()[0..8], magic_b[0..8]);
        }

        // check that the datapool does not open if the user version is incorrect
        {
            assert!(FileBackedMemory::open(&path, 2 * PAGE_SIZE, 1).is_err());
        }
    }
}
