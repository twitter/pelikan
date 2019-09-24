extern crate bindgen;

use std::env;
use std::fs;
use std::path::PathBuf;

fn get_cmake_binary_dir() -> String {
    use std::env::VarError;

    match env::var("CMAKE_BINARY_DIR") {
        Ok(var) => var,
        Err(e) => match e {
            VarError::NotPresent => panic!("CMAKE_BINARY_DIR environment variable was not set!"),
            VarError::NotUnicode(_) => panic!("CMAKE_BINARY_DIR contained invalid unicode!"),
        },
    }
}

fn main() {
    let cwd = env::current_dir().unwrap();
    eprintln!("CWD: {}", cwd.to_str().unwrap());

    let ccommon_include = fs::canonicalize("./../../../../deps/ccommon/include").unwrap();
    let include_path = fs::canonicalize("./..").unwrap();

    eprintln!("ccommon_include: {}", ccommon_include.to_str().unwrap());
    eprintln!("include_path: {}", include_path.to_str().unwrap());

    let cmake_binary_dir = get_cmake_binary_dir();

    let cbd = PathBuf::from(cmake_binary_dir);

    let mut config_h_dir = cbd.clone();
    config_h_dir.push("ccommon");

    let bindings = bindgen::Builder::default()
        .clang_args(vec![
            "-I", include_path.to_str().unwrap(),
            "-I", config_h_dir.to_str().unwrap(),
            "-I", ccommon_include.to_str().unwrap(),
        ])
        .header("wrapper.h")
        .blacklist_type("max_align_t") // https://github.com/rust-lang-nursery/rust-bindgen/issues/550
        .generate()
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("ffigen.rs"))
        .expect("Couldn't write bindings!");
}
