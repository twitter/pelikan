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

use libc::{fclose, fopen, FILE};
use std::error::Error;
use std::ffi::CString;
use std::fmt;

pub struct FileHandle {
    fp: *mut FILE,
}

impl FileHandle {
    pub fn open(path: impl AsRef<str>, mode: impl AsRef<str>) -> Result<Self, FileOpenError> {
        let path: CString = CString::new(path.as_ref().as_bytes().to_owned())
            .map_err(FileOpenError::InvalidName)?;
        let mode: CString = CString::new(mode.as_ref().as_bytes().to_owned())
            .map_err(FileOpenError::InvalidMode)?;

        let fp: *mut FILE = unsafe { fopen(path.as_ptr(), mode.as_ptr()) };

        if fp.is_null() {
            return Err(FileOpenError::UnableToOpenFile);
        }

        Ok(Self { fp })
    }

    pub fn handle(&self) -> *mut FILE {
        self.fp
    }
}

impl Drop for FileHandle {
    fn drop(&mut self) {
        unsafe { fclose(self.fp) };
    }
}

#[derive(Debug)]
pub enum FileOpenError {
    InvalidName(std::ffi::NulError),
    InvalidMode(std::ffi::NulError),
    UnableToOpenFile,
}

impl fmt::Display for FileOpenError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "Unable to open file")
    }
}
impl Error for FileOpenError {}
