extern crate cdb_rs;
extern crate cdbgen;
extern crate clap;
extern crate tempfile;

use clap::{App, Arg};
use std::path::PathBuf;

fn main() {
    let matches = App::new("cdbgen")
        .version("0.1.0")
        .author("Jonathan Simms")
        .about("Creates a cdb file with n-byte keys for testing")
        .arg(
            Arg::with_name("OUTPUT")
                .help("path to write cdb to")
                .required(true)
                .index(1),
        )
        .get_matches();

    let output = PathBuf::from(matches.value_of("OUTPUT").unwrap());
    cdbgen::create(&output).unwrap();
}
