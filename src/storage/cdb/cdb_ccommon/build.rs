extern crate bindgen;

use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::fs::File;
use std::io::prelude::*;

fn get_cmake_binary_dir() -> io::Result<String> {
    // this file is written by cmake on each run, updated with the location of
    // the build directory.
    let mut fp = fs::File::open("CMAKE_BINARY_DIR")?;
    let mut buf = String::new();
    let n = fp.read_to_string(&mut buf)?;
    assert!(n > 0, "file was empty");
    Ok(buf.trim_right().to_owned())
}

fn main() {
    println!("cargo:rustc-link-lib-static=ccommon");

    let include_path = fs::canonicalize("./../../../../deps/ccommon/include").unwrap();

    let cmake_binary_dir = get_cmake_binary_dir().unwrap();

    let cbd = PathBuf::from(cmake_binary_dir);

    let mut config_h_dir = cbd.clone();
    config_h_dir.push("ccommon");

    let mut lib_dir = cbd.clone();
    lib_dir.push("lib");

    println!("cargo:rustc-link-search-native={}", lib_dir.to_str().unwrap());

    // XXX: assume _build dir at pelikan top level

    let bindings = bindgen::Builder::default()
        .clang_args(vec![
            "-I", include_path.to_str().unwrap(),
            "-I", config_h_dir.to_str().unwrap(),
            "-L", lib_dir.to_str().unwrap(),
        ])
        .header("wrapper.h")
        .blacklist_type("max_align_t") // https://github.com/rust-lang-nursery/rust-bindgen/issues/550
        .generate()
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}

