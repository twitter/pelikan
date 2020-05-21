// ccommon - a cache common library.
// Copyright (C) 2019 Twitter, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use libc::{
    c_long, c_void, feof, fflush, fread, fseek, ftell, fwrite, FILE, SEEK_CUR, SEEK_END, SEEK_SET,
};
use std::io::{Error, ErrorKind, Read, Result, Seek, SeekFrom, Write};

/// A reference to a C `FILE` instance.
///
/// This is a convenience wrapper that supports `Read`, `Write`
/// and `Seek` from [`std::io`][io]. It does not take any ownership
/// over the `FILE` pointer.
///
/// [io]: std::io
pub struct CFileRef {
    _ptr: FILE,
}

impl CFileRef {
    /// Create a new reference from a const file pointer.
    ///
    /// # Safety
    /// It is undefined behaviour
    /// - To call this function with an invalid pointer.
    /// - If the reference created through this function outlives
    ///   the original pointer
    /// - If the pointer is modified while the reference is still live.
    ///
    /// # Panics
    /// This function will panic if `ptr` is null.
    pub unsafe fn from_ptr<'a>(ptr: *const FILE) -> &'a Self {
        assert!(!ptr.is_null());

        &*(ptr as *const Self)
    }

    /// Create a new reference from a mutable file pointer.
    ///
    /// # Safety
    /// It is undefined behaviour
    /// - To call this function with an invalid pointer.
    /// - If the reference created through this function outlives
    ///   the original pointer
    /// - If the pointer is modified while the reference is still live.
    ///
    /// # Panics
    /// This function will panic if `ptr` is null.
    pub unsafe fn from_ptr_mut<'a>(ptr: *mut FILE) -> &'a mut Self {
        assert!(!ptr.is_null());

        &mut *(ptr as *mut Self)
    }

    /// Convert the reference back to a `FILE` pointer.
    pub fn as_ptr(&self) -> *const FILE {
        self as *const Self as *const FILE
    }

    /// Convert the reference back to a `FILE` pointer.
    pub fn as_mut_ptr(&mut self) -> *mut FILE {
        self as *mut Self as *mut FILE
    }
}

impl Write for CFileRef {
    fn write(&mut self, bytes: &[u8]) -> Result<usize> {
        let written = unsafe {
            fwrite(
                bytes.as_ptr() as *const c_void,
                1,
                bytes.len(),
                self.as_mut_ptr(),
            )
        };

        if written != bytes.len() {
            return Err(Error::last_os_error());
        }

        Ok(written)
    }

    fn flush(&mut self) -> Result<()> {
        let res = unsafe { fflush(self.as_mut_ptr()) };

        if res != 0 {
            Err(Error::last_os_error())
        } else {
            Ok(())
        }
    }
}

impl Read for CFileRef {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let read = unsafe {
            fread(
                buf.as_mut_ptr() as *mut c_void,
                1,
                buf.len(),
                self.as_mut_ptr(),
            )
        };

        if read == buf.len() {
            return Ok(read);
        }

        if unsafe { feof(self.as_mut_ptr()) != 0 } {
            return Ok(read);
        }

        Err(Error::last_os_error())
    }
}

impl Seek for CFileRef {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64> {
        use std::convert::TryInto;

        let (offset, origin) = match pos {
            SeekFrom::Start(offset) => (offset as i64, SEEK_SET),
            SeekFrom::Current(offset) => (offset, SEEK_CUR),
            SeekFrom::End(offset) => (offset, SEEK_END),
        };

        let offset: c_long = match offset.try_into() {
            Ok(off) => off,
            Err(e) => return Err(Error::new(ErrorKind::Other, Box::new(e))),
        };

        let ret = unsafe { fseek(self.as_mut_ptr(), offset, origin) };
        if ret != 0 {
            return Err(Error::last_os_error());
        }

        let off = unsafe { ftell(self.as_mut_ptr()) };
        if off < 0 {
            return Err(Error::last_os_error());
        }

        Ok(off as u64)
    }
}
