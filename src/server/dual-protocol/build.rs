use std::env;
use std::path::Path;

fn main() {
    let flag = if cfg!(target_os = "linux") {
        "OS_LINUX"
    } else {
        "OS_DARWIN"
    };

    let bindir = get_cmake_binary_dir();

    cc::Build::new()
        .include("../../")
        .include("../../../deps/ccommon/include")
        .include(&bindir)
        .file("memcached/process.c")
        .static_flag(true)
        .define(flag, "1")
        .compile("memcache");

    let bindings = bindgen::builder()
        .clang_arg("-I../../")
        .clang_arg("-I../../../deps/ccommon/include")
        .clang_arg("-I".to_string() + &bindir)
        .header("memcached/process.h")
        .derive_default(true)
        .derive_copy(true)
        .derive_debug(true)
        .prepend_enum_name(false)
        .constified_enum(".*")
        .blacklist_type("buf")
        .blacklist_type("metric")
        .blacklist_type("option")
        .whitelist_type("process_.*")
        .whitelist_function("process_.*")
        .whitelist_function("twemcache_.*")
        .whitelist_var("ALLOW_FLUSH")
        .whitelist_var("PREFILL.*")
        .generate()
        .expect("Failed to generate bindings");

    let out_dir = env::var_os("OUT_DIR").unwrap();
    let out_dir = Path::new(&out_dir);
    bindings
        .write_to_file(out_dir.join("memcached.rs"))
        .unwrap();
}

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
