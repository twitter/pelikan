extern crate bindgen;

use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    println!("cargo:rustc-link-lib-static=ccommon");

    let include_path = fs::canonicalize("./../../../../deps/ccommon/include").unwrap();
    let config_h_dir = fs::canonicalize("./../../../../_build/ccommon").unwrap();
    let lib_dir = fs::canonicalize("./../../../../_build/ccommon/lib").unwrap();

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

