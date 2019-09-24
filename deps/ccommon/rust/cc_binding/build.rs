extern crate bindgen;
#[macro_use]
extern crate failure;

use std::env;
use std::ffi::OsString;
use std::fs;
use std::io;
use std::io::prelude::*;
use std::io::BufReader;
use std::os::unix::fs as unix_fs;
use std::path::Path;
use std::path::PathBuf;
use std::result;

type Result<T> = result::Result<T, failure::Error>;

const CMAKE_CACHE: &str = "CMakeCache.txt";
const CCOMMON_BINARY_DIR_KEY: &str = "ccommon_BINARY_DIR:STATIC";

fn get_cmake_binary_dir() -> io::Result<PathBuf> {
    use std::env::VarError;

    match env::var("CMAKE_BINARY_DIR") {
        Ok(var) => Ok(var.into()),
        Err(e) => {
            match e {
                VarError::NotPresent => panic!(
                    "CMAKE_BINARY_DIR environment variable was not set!"
                ),
                VarError::NotUnicode(v) => Ok(PathBuf::from(v))
            }
        }
    }
}

fn get_cmake_cache_value(binary_dir: &Path, key: &str) -> Result<Option<String>> {
    let cache_path = binary_dir.join(CMAKE_CACHE);
    let fp = BufReader::new(fs::File::open(cache_path)?);

    for x in fp.lines() {
        let line = x?;
        let needle = format!("{}=", key);
        if line.starts_with(&needle[..]) {
            if let Some(v) = line.rsplit("=").take(1).last() {
                return Ok(Some(v.to_owned()));
            } else {
                bail!("bad line: {:#?}", line);
            }
        }
    }

    Ok(None)
}

#[allow(unused)]
fn dump_env() {
    let mut kvs: Vec<(String, String)> = ::std::env::vars().collect();
    kvs.sort();
    eprintln!("-----<( ENVIRONMENT )>-----");
    for (k, v) in kvs {
        eprintln!("{}: {}", k, v);
    }
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
                format!(
                    "could not find {} in {}",
                    CCOMMON_BINARY_DIR_KEY, CMAKE_CACHE
                )
                .as_ref(),
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
            "-I",
            include_path.to_str().unwrap(),
            "-I",
            config_h_dir.to_str().unwrap(),
            "-I",
            cbd.to_str().unwrap(),
            "-L",
            &lib_dir,
        ])
        .header("wrapper.h")
        .prepend_enum_name(false)
        .whitelist_recursively(false)

        // All public types, variables and, functions exported by ccommon
        .whitelist_type("u?intmax_t")
        .whitelist_type("u?int([0-9]+)_t")

        .whitelist_type("cc_.*")
        .whitelist_function("cc_.*")
        .whitelist_var("CC_.*")
        .whitelist_function("_cc_.*")
        .whitelist_type("rstatus_i")
        .whitelist_type("err_i")
        
        .whitelist_type("buf_.*")
        .whitelist_type("buf")
        .whitelist_var("BUF_.*")
        .whitelist_function("buf_.*")

        .whitelist_type("dbuf_.*")
        .whitelist_function("dbuf_.*")
        .whitelist_var("DBUF_.*")

        .whitelist_type("channel_.*")
        .whitelist_var("CHANNEL_.*")
        .whitelist_type("ch_id_i")
        .whitelist_type("ch_level_e")
        
        .whitelist_var("PIPE_.*")
        .whitelist_type("pipe_.*")
        .whitelist_function("pipe_.*")

        .whitelist_var("TCP_.*")
        .whitelist_type("tcp_.*")
        .whitelist_function("tcp_.*")

        .whitelist_function("hash_murmur3_.*")

        .whitelist_type("log_.*")
        .whitelist_function("log_.*")
        .whitelist_var("LOG_.*")
        
        .whitelist_type("sockio_.*")
        .whitelist_function("sockio_.*")

        .whitelist_type("duration")
        .whitelist_type("duration_.*")
        .whitelist_type("timeout")
        .whitelist_function("duration_.*")
        .whitelist_function("timeout_.*")

        .whitelist_type("timing_wheel_.*")
        .whitelist_type("timing_wheel")
        .whitelist_function("timing_wheel_.*")
        .whitelist_type("tevent_tqh")
        .whitelist_type("timeout_.*")

        .whitelist_var("NELEM_DELTA")
        .whitelist_type("array_.*")
        .whitelist_type("array")
        .whitelist_function("array_.*")

        .whitelist_type("bstring")
        .whitelist_function("bstring_.*")

        .whitelist_var("DEBUG_.*")
        .whitelist_type("debug_.*")
        .whitelist_function("debug_.*")
        .whitelist_var("dlog")
        .whitelist_function("_log")
        .whitelist_function("_log_hexdump")

        .whitelist_var("EVENT_.*")
        .whitelist_type("event_.*")
        .whitelist_function("event_.*")

        .whitelist_type("logger")
        .whitelist_type("log_.*")
        .whitelist_function("log_.*")
        .whitelist_function("_log_fd")

        .whitelist_type("metric")
        .whitelist_type("metric_.*")
        .whitelist_function("metric_.*")

        .whitelist_var("OPTLINE_MAXLEN")
        .whitelist_var("OPTNAME_MAXLEN")
        .whitelist_var("OPTVAL_MAXLEN")
        .whitelist_type("option_.*")
        .whitelist_var("option_.*")
        .whitelist_type("option")
        .whitelist_function("option_.*")

        .whitelist_type("rbuf_.*")
        .whitelist_type("rbuf")
        .whitelist_function("rbuf_.*")

        .whitelist_var("RING_ARRAY_.*")
        .whitelist_type("ring_array")
        .whitelist_function("ring_array_.*")

        .whitelist_type("sig_fn")
        .whitelist_type("signal")
        .whitelist_var("signals")
        .whitelist_function("signal_.*")

        .whitelist_type("stats_.*")
        .whitelist_function("stats_.*")

        .whitelist_type("iobuf_p")
        .whitelist_type("io_.*")
        .whitelist_type("stream_.*")
        .whitelist_type("address_p")
        .whitelist_var("BUFSOCK_.*")

        .derive_copy(true)
        .derive_debug(true)
        .generate()
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");

    // ./target/debug/build/cc_binding-27eac70f0fa2e180/out  <<- starts here

    // cc_binding-27eac70f0fa2e180
    let symlink_content = out_path.parent().unwrap().file_name().unwrap();

    let build_dir = out_path.parent().and_then(|p| p.parent()).unwrap();

    let link_location = build_dir.join("cc_binding");
    let _ = fs::remove_file(link_location.as_path());
    unix_fs::symlink(symlink_content, link_location).unwrap();
}
