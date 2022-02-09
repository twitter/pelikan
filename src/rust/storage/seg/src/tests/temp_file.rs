// Code taken from https://andrewra.dev/2019/03/01/testing-in-rust-temporary-files/
// Structure that allows use of temporary files in `tests.rs`

use std::path::PathBuf;
use std::env::*;
use tempfile::*;

pub struct TempFile {
    path: PathBuf,
    _tempdir: TempDir,
}

impl TempFile {
    // Create a temporary file
    pub fn create(filename: &str) -> Self {

        // First, figure out the right file in `tests/fixtures/`:
        let root_dir = &std::env::var("CARGO_MANIFEST_DIR").expect("$CARGO_MANIFEST_DIR");
        let mut source = PathBuf::from(root_dir);
        source.push("tests/temp_files");
        source.push(&filename);

        // The "real" path of the file is going to be under a temporary directory
        let tempdir = tempfile::tempdir().unwrap();
        let mut path = PathBuf::from(&tempdir.path());
        path.push(&filename);

        println!("{:?}, {:?}", tempdir, path);

        TempFile { _tempdir: tempdir, path }
    }

    // Return the path
    pub fn path(self) -> PathBuf {
        self.path
    }
}
