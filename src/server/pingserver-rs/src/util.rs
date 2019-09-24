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
