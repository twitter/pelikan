extern crate bindgen;
#[macro_use]
extern crate failure;

use std::env;
use std::fs;
use std::io;
use std::io::BufReader;
use std::io::prelude::*;
use std::ffi::OsString;
use std::os::unix::fs as unix_fs;
use std::path::Path;
use std::path::PathBuf;
use std::result;

type Result<T> = result::Result<T, failure::Error>;

fn get_cmake_binary_dir() -> io::Result<String> {
    // this file is written by cmake on each run, updated with the location of
    // the build directory.
    let mut fp = fs::File::open("../CMAKE_BINARY_DIR")?;
    let mut buf = String::new();
    let n = fp.read_to_string(&mut buf)?;
    assert!(n > 0, "file was empty");
    Ok(String::from(buf.trim_right()))
}

const CMAKE_CACHE: &str = "CMakeCache.txt";
const CCOMMON_BINARY_DIR_KEY: &str = "ccommon_BINARY_DIR:STATIC";

fn get_cmake_cache_value(binary_dir: &Path, key: &str) -> Result<Option<String>> {
    let cache_path = binary_dir.join(CMAKE_CACHE);
    let fp = BufReader::new(fs::File::open(cache_path)?);

    for x in fp.lines() {
        let line = x?;
        let needle = format!("{}=", key);
        if line.starts_with(&needle[..]) {
            if let Some(v) = line.rsplit("=").take(1).last() {
                return Ok(Some(v.to_owned()))
            } else {
                bail!("bad line: {:#?}", line);
            }
        }
    }

    Ok(None)
}

fn main() {
    println!("cargo:rustc-link-lib=static=ccommon-2.1.0");
    if cfg!(target_os = "macos") {
        println!("cargo:rustc-link-lib=framework=Security");
    }

    let include_path = fs::canonicalize("./../../include").unwrap();

    let cmake_binary_dir = match get_cmake_binary_dir() {
        Ok(p) => p,
        Err(err) => panic!("Failed locating the CMAKE_BINARY_DIR file: {:#?}", err),
    };

    let cbd = PathBuf::from(cmake_binary_dir);

    let mut config_h_dir = cbd.clone();
    config_h_dir.push("ccommon");

    let lib_dir: String = {
        let cbd = get_cmake_cache_value(&cbd, CCOMMON_BINARY_DIR_KEY)
            .map(|o| o.map(OsString::from))
            .unwrap()
            .expect(
                format!("could not find {} in {}", CCOMMON_BINARY_DIR_KEY, CMAKE_CACHE).as_ref()
            );

        let cbd = Path::new(&cbd);
        cbd.join("lib").to_str().unwrap().to_string()
    };

    println!("cargo:rustc-link-search=native={}", lib_dir);

    if cfg!(target_os = "macos") {
        println!("cargo:rustc-link-search=framework=/System/Library/Frameworks");
    }

    let bindings = bindgen::Builder::default()
        .clang_args(vec![
            "-I", include_path.to_str().unwrap(),
            "-I", config_h_dir.to_str().unwrap(),
            "-I", cbd.to_str().unwrap(),
            "-L", &lib_dir,
        ])
        .header("wrapper.h")
        .blacklist_type("max_align_t") // https://github.com/rust-lang-nursery/rust-bindgen/issues/550
        .generate()
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");

    // ./target/debug/build/cc_binding-27eac70f0fa2e180/out  <<- starts here

    // cc_binding-27eac70f0fa2e180
    let symlink_content =
        out_path.parent().unwrap().file_name().unwrap();

    let build_dir = out_path.parent().and_then(|p| p.parent()).unwrap();

    let link_location = build_dir.join("cc_binding");
    let _ = fs::remove_file(link_location.as_path());
    unix_fs::symlink(symlink_content, link_location).unwrap();
}

