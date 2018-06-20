extern crate bytes;
extern crate rand;
extern crate memmap;
extern crate itertools;

#[macro_use] extern crate log;
extern crate env_logger;
extern crate tempfile;

#[macro_use] extern crate proptest;

extern crate tinycdb;
extern crate clap;
extern crate crypto;
extern crate libc;

pub mod cdb;

pub use cdb::cdb_c::{cdb_rs_create, cdb_rs_destroy, CDBHandle};
