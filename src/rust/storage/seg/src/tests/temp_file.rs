// Code taken from https://andrewra.dev/2019/03/01/testing-in-rust-temporary-files/
// Structure that allows use of temporary files in `tests.rs`

use std::path::PathBuf;
use std::env::*;
use std::fs::File;
use std::io::{self, Write};
use tempfile::*;

pub struct TempFile {
    path: PathBuf,
    _tempdir: TempDir,
}

impl TempFile {
    // Create a temporary file
    pub fn create(filename: &str) -> Self {

        // The "real" path of the file is going to be under a temporary directory
        let tempdir = tempfile::tempdir().unwrap();
        let path = tempdir.path().join(filename);
        // let mut file = File::create(&path).expect("failed to create file");
        // writeln!(file, "Cassy was here. Briefly.").expect("failed to write to file");

        //let mut path = PathBuf::from(&tempdir.path());
        //path.push(&filename);

        println!("{:?}, {:?}", tempdir, &path);

        TempFile { _tempdir: tempdir, path }
    }

    // Return the path
    pub fn path(self) -> PathBuf {
        self.path
    }
}
