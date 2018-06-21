extern crate bytes;
extern crate memmap;
extern crate itertools;
extern crate rand;

#[macro_use] extern crate log;
extern crate env_logger;
extern crate tempfile;

#[macro_use] extern crate proptest;

extern crate tinycdb;
extern crate clap;
extern crate crypto;
extern crate libc;

pub mod cdb;

pub use cdb::{CDB, Result, CDBError};


