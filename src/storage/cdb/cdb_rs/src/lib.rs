extern crate bytes;
extern crate memmap;

#[macro_use] extern crate log;
extern crate env_logger;
extern crate clap;

// dev dependencies
#[cfg(test)] extern crate tempfile;
#[cfg(test)] extern crate tinycdb;

pub mod cdb;
pub use cdb::{CDB, Result, CDBError};
pub use memmap::Mmap;
