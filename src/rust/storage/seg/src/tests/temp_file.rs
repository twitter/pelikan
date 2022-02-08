// Code taken from https://andrewra.dev/2019/03/01/testing-in-rust-temporary-files/
// Structure that allows use of temporary files in `tests.rs`

use std::path::PathBuf;
use tempfile::*;

pub struct TempFile {
    path: PathBuf,
    _tempdir: TempDir,
}

impl TempFile {
    // Create a temporary file
    pub fn create(fixture_filename: &str) -> Self {
        // The path of the file is going to be under a temporary directory
        let tempdir = tempfile::tempdir().unwrap();
        let mut path = PathBuf::from(&tempdir.path());
        path.push(&fixture_filename);

        println!("{:?}, {:?}", tempdir, path);

        TempFile { _tempdir: tempdir, path }
    }

    // Return the path
    pub fn path(self) -> PathBuf {
        self.path
    }
}
