extern crate bytes;
extern crate cc_binding;
extern crate ccommon_rs;
extern crate clap;
extern crate env_logger;
extern crate failure;
#[macro_use]
extern crate failure_derive;
extern crate libc;
#[macro_use]
extern crate log;
extern crate memmap;

// dev dependencies
#[cfg(test)]
extern crate tempfile;

pub mod cdb;
pub use cdb::{cdb_handle, CDBError, Reader, Result, Writer};
pub use memmap::Mmap;
