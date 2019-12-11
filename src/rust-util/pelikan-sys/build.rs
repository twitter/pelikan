// Copyright (C) 2019 Twitter, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::env;
use std::path::PathBuf;

use bindgen;
use lazy_static::lazy_static;

lazy_static! {
    static ref INCLUDE_PATH: Vec<String> = vec![
        "../../../deps/ccommon/include".to_owned(),
        "../../../src".to_owned(),
        get_cmake_binary_dir()
    ];
}

fn print_directives(lib: impl AsRef<str>, subdir: impl AsRef<str>) {
    let lib: &str = lib.as_ref();
    let subdir: &str = subdir.as_ref();
    let bindir = get_cmake_binary_dir();

    println!("cargo:rustc-link-search={}/{}/", bindir, subdir);
    println!("cargo:rustc-link-lib={}", lib);
}

fn builder() -> bindgen::Builder {
    let mut builder = bindgen::builder();

    for arg in &*INCLUDE_PATH {
        builder = builder.clang_args(vec!["-I", &*arg]);
    }

    builder
        .constified_enum(".*")
        .derive_default(true)
        .derive_copy(true)
        .derive_debug(true)
        .prepend_enum_name(false)
        .whitelist_type("u?int([0-9]+|max)_t")
        // blacklist types from ccommon
        .blacklist_item("option")
        .blacklist_item("metric")
        .blacklist_item("pipe_conn")
        .blacklist_item("ring_array")
        .blacklist_item("timeout_cb_fn")
        .blacklist_item("timeout_event")
        .blacklist_item("buf")
        .blacklist_item("bstring")
        .blacklist_item("rstatus_i")
        .blacklist_item("array")
        .blacklist_item("option_val")
        .blacklist_item("option_type")
        .blacklist_item("option_val_u")
        .blacklist_item("option_type_e")
        // blacklist types from libc
        .blacklist_item("addrinfo")
        .blacklist_item("time_t")
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

fn gen_client_network() {
    print_directives("client-network", "client/network");

    let bindings = builder()
        .header("../../client/network/cli_network.h")
        .whitelist_type("cli_network")
        .whitelist_type("cli_network_e")
        .whitelist_type("network_config")
        .whitelist_var("tcp_handler")
        .whitelist_var("network_config")
        .whitelist_function("cli_connect")
        .whitelist_function("cli_disconnect")
        .whitelist_function("cli_reconnect")
        .whitelist_var("PROMPT_FMT_OFFLINE")
        .whitelist_var("PROMPT_FMT_LOCAL")
        .whitelist_var("PROMPT_FMT_REMOTE")
        .whitelist_var("SEND_ERROR")
        .whitelist_var("RECV_ERROR")
        .whitelist_var("RECV_HUP")
        .whitelist_var("DISCONNECT_MSG")
        .whitelist_var("RECONNECT_MSG")
        .generate()
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("client-network.rs"))
        .expect("Couldn't write bindings");
}

fn gen_util() {
    print_directives("util", "util");

    let bindings = builder()
        .header("../../util/util.h")
        .header("../../util/procinfo.h")
        // procinfo
        .whitelist_type("procinfo_metrics_st")
        .whitelist_function("procinfo_setup")
        .whitelist_function("procinfo_teardown")
        .whitelist_function("procinfo_update")
        // util
        .whitelist_function("daemonize")
        .whitelist_function("show_version")
        .whitelist_function("getaddr")
        .whitelist_function("create_pidfile")
        .whitelist_function("remove_pidfile")
        .generate()
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("util.rs"))
        .expect("Couldn't write bindings");
}
fn gen_time() {
    print_directives("time", "time");

    let bindings = builder()
        .header("../../time/time.h")
        .whitelist_type("proc_time_i")
        .whitelist_type("proc_time_fine_i")
        .whitelist_type("delta_time_i")
        .whitelist_type("delta_time_fine_i")
        .whitelist_type("unix_time_u")
        .whitelist_type("unix_time_fine_u")
        .whitelist_type("memcache_time_u")
        .whitelist_type("memcache_time_fine_u")
        .whitelist_type("time_i")
        .whitelist_type("time_fine_i")
        .whitelist_var("TIME_.*")
        .whitelist_type("time_options_st")
        .whitelist_var("time_start")
        .whitelist_var("proc_sec")
        .whitelist_var("proc_ms")
        .whitelist_var("proc_us")
        .whitelist_var("proc_ns")
        .whitelist_var("time_type")
        .whitelist_var("NSEC_PER_USEC")
        .whitelist_var("USEC_PER_SEC")
        .whitelist_var("MSEC_PER_SEC")
        .whitelist_function("time_update")
        .whitelist_function("time_setup")
        .whitelist_function("time_teardown")
        .generate()
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("time.rs"))
        .expect("Couldn't write bindings");
}
fn gen_core() {
    print_directives("core", "core");

    let bindings = builder()
        .header("../../core/core.h")
        .blacklist_item("ADMIN_PORT")
        .blacklist_item("SERVER_PORT")
        .whitelist_function("core_.*")
        .whitelist_var("ADMIN_.*")
        .whitelist_type("event_base")
        .whitelist_type("context")
        .whitelist_var("admin_init")
        .whitelist_var("server_init")
        .whitelist_var("worker_init")
        .whitelist_type("admin_options_st")
        .whitelist_var("pipe_new")
        .whitelist_var("pipe_term")
        .whitelist_var("conn_new")
        .whitelist_var("conn_term")
        .whitelist_var("SERVER_.*")
        .whitelist_type("server_options_st")
        .whitelist_type("server_metrics_st")
        .whitelist_var("WORKER_.*")
        .whitelist_type("worker_options_st")
        .whitelist_type("worker_metrics_st")
        .whitelist_type("data_processor")
        .whitelist_type("data_fn")
        .generate()
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("core.rs"))
        .expect("Couldn't write bindings");
}
fn gen_hotkey() {
    print_directives("hotkey", "hotkey");

    let bindings = builder()
        .header("../../hotkey/hotkey.h")
        .header("../../hotkey/key_window.h")
        .header("../../hotkey/kc_map.h")
        .header("../../hotkey/constant.h")
        .whitelist_function("key_window_.*")
        .whitelist_function("kc_map_.*")
        .whitelist_function("hotkey_.*")
        .whitelist_type("hotkey_.*")
        .whitelist_var("HOTKEY_.*")
        .whitelist_var("MAX_KEY_LEN")
        .generate()
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("hotkey.rs"))
        .expect("Couldn't write bindings");
}
fn gen_datapool() {
    print_directives("datapool", "datapool");

    let bindings = builder()
        .header("../../datapool/datapool.h")
        .whitelist_type("datapool")
        .whitelist_function("datapool_.*")
        .generate()
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("datapool.rs"))
        .expect("Couldn't write bindings");
}

fn gen_ds_bitmap() {
    print_directives("ds_bitmap", "data_structure/bitmap");

    let bindings = builder()
        .header("../../data_structure/bitmap/bitset.h")
        .whitelist_var("BITSET_COL_MAX")
        .whitelist_type("bitset")
        .whitelist_function("bitset_init")
        .whitelist_function("bitset_get")
        .whitelist_function("bitset_set")
        .generate()
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("ds_bitmap.rs"))
        .expect("Couldn't write bindings");
}
fn gen_ds_sarray() {
    print_directives("ds_sarray", "data_structure/sarray");

    let bindings = builder()
        .header("../../data_structure/sarray/sarray.h")
        .whitelist_type("sarray_p")
        .whitelist_type("sarray_rstatus_e")
        .whitelist_function("sarray_.*")
        .whitelist_recursively(false)
        .derive_default(true)
        .derive_copy(true)
        .derive_debug(true)
        .generate()
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("ds_sarray.rs"))
        .expect("Couldn't write bindings");
}
fn gen_ds_ziplist() {
    print_directives("ds_ziplist", "data_structure/ziplist");

    let bindings = builder()
        .header("../../data_structure/ziplist/ziplist.h")
        .blacklist_item("blob")
        .whitelist_type("ziplist_.*")
        .whitelist_type("zipentry_p")
        .whitelist_function("ziplist_.*")
        .whitelist_function("zipentry_.*")
        .generate()
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("ds_ziplist.rs"))
        .expect("Couldn't write bindings");
}

fn gen_cuckoo() {
    print_directives("cuckoo", "storage/cuckoo");

    let bindings = builder()
        .header("../../storage/cuckoo/cuckoo.h")
        .blacklist_item("delta_time_i")
        .blacklist_item("proc_time_i")
        .whitelist_var("CUCKOO_.*")
        .whitelist_type("cuckoo_.*")
        .whitelist_function("cuckoo_.*")
        .whitelist_var("cas_enabled")
        .whitelist_var("cas_val")
        .whitelist_var("max_ttl")
        .whitelist_type("val_type")
        .whitelist_type("val_type_t")
        .whitelist_type("val")
        .whitelist_type("item")
        .whitelist_var("KEY_MAXLEN")
        .whitelist_var("CAS_VAL_MIN")
        .whitelist_var("CAS_LEN")
        .whitelist_var("MIN_ITEM_CHUNK_SIZE")
        .whitelist_var("ITEM_HDR_SIZE")
        .whitelist_var("ITEM_CAS_POS")
        .whitelist_var("ITEM_KEY_POS")
        .whitelist_var("ITEM_VAL_POS")
        .whitelist_var("ITEM_OVERHEAD")
        .whitelist_function("item_.*")
        .generate()
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("cuckoo.rs"))
        .expect("Couldn't write bindings");
}
fn gen_slab() {
    print_directives("slab", "storage/slab");

    let bindings = builder()
        .header("../../storage/slab/slab.h")
        .blacklist_type("proc_time_i")
        .whitelist_var("SLAB_.*")
        .whitelist_var("ITEM_.*")
        .whitelist_var("EVICT_.*")
        .whitelist_type("slab_.*")
        .whitelist_type("slab")
        .whitelist_type("slab_tqh")
        .whitelist_var("hash_table")
        .whitelist_var("slab_size")
        .whitelist_var("slab_metrics")
        .whitelist_function("slab_.*")
        .whitelist_type("item")
        .whitelist_type("item_.*")
        .whitelist_var("use_cas")
        .whitelist_var("cas_id")
        .whitelist_var("slab_profile")
        .whitelist_function("item_.*")
        .whitelist_type("hash_table")
        .whitelist_function("hashtable_.*")
        .whitelist_type("slabclass")
        .whitelist_var("SLABCLASS_.*")
        .whitelist_var("slabclass")
        .whitelist_var("perslab")
        .generate()
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("slab.rs"))
        .expect("Couldn't write bindings");
}
fn gen_cdb() {
    // No dependencies since the CDB backend is in rust.

    let bindings = builder()
        .header("../../storage/cdb/cdb_rs.h")
        .whitelist_type("cdb_.*")
        .whitelist_function("cdb_.*")
        .generate()
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("cdb.rs"))
        .expect("Couldn't write bindings");
}

fn gen_protocol_resp() {
    print_directives("protocol_resp", "protocol/data/resp");

    let bindings = builder()
        .header("../../protocol/data/resp_include.h")
        .whitelist_type("bitmap_elem(_e)?")
        .whitelist_type("list_elem(_e)?")
        .whitelist_type("sarray_elem(_e)?")
        .whitelist_type("compose_.*")
        .whitelist_function("compose_.*")
        .whitelist_type("parse_.*")
        .whitelist_function("parse_.*")
        .whitelist_function("process_request")
        .whitelist_type("response_.*")
        .whitelist_var("RSP_.*")
        .whitelist_type("response")
        .whitelist_type("response_.*")
        .whitelist_type("element")
        .whitelist_type("element_type(_e)?")
        .whitelist_function("token_.*")
        .whitelist_type("request")
        .whitelist_type("request_.*")
        .whitelist_var("REQ_.*")
        .whitelist_var("KEY_MAXLEN")
        .whitelist_type("command")
        .whitelist_var("command_table")
        .whitelist_type("cmd_type(_e)?")
        .whitelist_function("request_.*")
        .generate()
        .expect("Unable to generate bindings");

    let out_path = env::var("OUT_DIR").map(PathBuf::from).unwrap();
    bindings
        .write_to_file(out_path.join("protocol_resp.rs"))
        .expect("Couldn't write bindings");
}
fn gen_protocol_resp_tw() {
    // TODO: This protocol doesn't even exist in pelikan yet
    print_directives("protocol_resp_tw", "protocol/data/resp_tw");

    let bindings = builder()
        .header("../../protocol/data/resp_tw_include.h")
        .whitelist_type("parse_rstatus(_e)?")
        .whitelist_type("compose_rstatus(_e)?")
        .whitelist_type("element_type(_e)?")
        .whitelist_type("element")
        .whitelist_function("token_.*")
        .whitelist_function("parse_.*")
        .whitelist_function("compose_.*")
        .whitelist_type("response_(options|metrics)_st")
        .whitelist_var("RSP_.*")
        .whitelist_type("attribuet_entry")
        .whitelist_type("response")
        .whitelist_function("response_.*")
        .whitelist_type("request_(options|metrics)_st")
        .whitelist_type("cmd_type(_e)?")
        .whitelist_type("command")
        .whitelist_var("command_table")
        .whitelist_type("request")
        .whitelist_function("request_.*")
        .whitelist_type("parse_(req|rsp)_metrics_st")
        .whitelist_type("compose_(req|rsp)_metrics_st")
        .whitelist_var("OPT_UNLIMITED")
        .whitelist_type("sarray_elem(_e)?")
        .whitelist_type("list_elem(_e)?")
        .whitelist_type("bitmap_elem(_e)?")
        .generate()
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("protocol_resp_tw.rs"))
        .expect("Couldn't write bindings");
}
fn gen_protocol_ping() {
    print_directives("protocol_ping", "protocol/data/ping");

    let bindings = builder()
        .header("../../protocol/data/ping_include.h")
        .whitelist_var("RESPONSE")
        .whitelist_var("RSP_LEN")
        .whitelist_var("REQUEST")
        .whitelist_var("REQ_LEN")
        .whitelist_type("compose_.*")
        .whitelist_function("compose_.*")
        .whitelist_type("parse_.*")
        .whitelist_function("parse_.*")
        .generate()
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("protocol_ping.rs"))
        .expect("Couldn't write bindings");
}
fn gen_protocol_memcache() {
    print_directives("protocol_memcache", "protocol/data/memcache");

    let bindings = builder()
        .header("../../protocol/data/memcache_include.h")
        .whitelist_type("response_.*")
        .whitelist_var("rsp_strings")
        .whitelist_type("response")
        .whitelist_function("response_.*")
        .whitelist_type("request_.*")
        .whitelist_type("request")
        .whitelist_function("request_.*")
        .whitelist_type("process_request")
        .whitelist_type("parse_.*")
        .whitelist_function("parse_.*")
        .whitelist_var("KLOG_.*")
        .whitelist_type("klog_.*")
        .whitelist_var("klog_enabled")
        .whitelist_function("klog_.*")
        .whitelist_function("_klog_write")
        .whitelist_var("MAX_.*")
        .whitelist_var("DATAFLAG_SIZE")
        .whitelist_type("compose_.*")
        .whitelist_function("compose_.*")
        .whitelist_function("REQ_.*")
        .generate()
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("protocol_memcache.rs"))
        .expect("Couldn't write bindings");
}
fn gen_protocol_admin() {
    print_directives("protocol_admin", "protocol/admin");

    let bindings = builder()
        .header("../../protocol/admin/admin_include.h")
        .whitelist_type("response")
        .whitelist_type("response_.*")
        .whitelist_function("admin_.*")
        .whitelist_type("request_.*")
        .whitelist_type("request")
        .whitelist_type("parse_.*")
        .whitelist_var("METRIC_.*")
        .whitelist_var("VERSION_PRINTED")
        .whitelist_function("print_stats")
        .whitelist_function("compose_.*")
        .whitelist_var("COMPOSE_.*")
        .whitelist_type("compose_.*")
        .generate()
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("protocol_admin.rs"))
        .expect("Couldn't write bindings");
}

fn main() {
    dump_env();

    // Cargo sometimes puts OUT_DIR in the same directory as
    // a previous build, so this ensures that old artifacts
    // don't stick around.
    std::process::Command::new("rm")
        .arg("-rf")
        .arg(format!("{}/*", env::var("OUT_DIR").unwrap()))
        .status()
        .expect("failed to remove previous build artifacts");

    println!("cargo:rerun-if-env-changed=CMAKE_BUILD_DIR");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rustc-link-lib=ccommon-2.1.0");
    println!(
        "cargo:rustc-link-search={}/ccommon/lib",
        get_cmake_binary_dir()
    );

    for entry in glob::glob("../../**/*.h").unwrap().filter_map(|x| x.ok()) {
        if let Some(entry) = entry.to_str() {
            println!("cargo:rerun-if-changed={}", entry);
        }
    }
    for entry in glob::glob("../../../deps/ccommon/include")
        .unwrap()
        .filter_map(|x| x.ok())
    {
        if let Some(entry) = entry.to_str() {
            println!("cargo:rerun-if-changed={}", entry);
        }
    }

    println!("cargo:rerun-if-changed={}/config.h", get_cmake_binary_dir());

    {
        let bindings = builder()
            .header("../../data_structure/shared.h")
            .whitelist_type("blob")
            .whitelist_type("blob_type")
            .whitelist_type("blob_type_t")
            .generate()
            .expect("Unable to generate bindings");

        let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
        bindings
            .write_to_file(out_path.join("ds_shared.rs"))
            .expect("Couldn't write bindings");
    }
    {
        let bindir = get_cmake_binary_dir();

        let bindings = builder()
            .header(format!("{}/config.h", bindir))
            .whitelist_recursively(true)
            // Only whitelist version identifiers since otherwise
            // changing the cmake config could cause compilation errors
            .whitelist_var("VERSION_.*")
            .generate()
            .expect("Unable to generate bindings");

        let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
        bindings
            .write_to_file(out_path.join("config.rs"))
            .expect("Couldn't write bindings");
    }

    // Data Strucutures
    if cfg!(feature = "ds_bitmap") {
        gen_ds_bitmap();
    }
    if cfg!(feature = "ds_sarray") {
        gen_ds_sarray();
    }
    if cfg!(feature = "ds_ziplist") {
        gen_ds_ziplist();
    }

    // Storages
    if cfg!(feature = "cuckoo") {
        gen_cuckoo();
    }
    if cfg!(feature = "slab") {
        gen_slab();
    }
    if cfg!(feature = "cdb") {
        gen_cdb();
    }

    // Protocols
    if cfg!(feature = "protocol_resp") {
        gen_protocol_resp();
    }
    if cfg!(feature = "protocol_resp_tw") {
        gen_protocol_resp_tw();
    }
    if cfg!(feature = "protocol_ping") {
        gen_protocol_ping();
    }
    if cfg!(feature = "protocol_memcache") {
        gen_protocol_memcache();
    }
    if cfg!(feature = "protocol_admin") {
        gen_protocol_admin();
    }

    if cfg!(feature = "client-network") {
        gen_client_network();
    }
    if cfg!(feature = "util") {
        gen_util();
    }
    if cfg!(feature = "time") {
        gen_time();
    }
    if cfg!(feature = "core") {
        gen_core();
    }
    if cfg!(feature = "hotkey") {
        gen_hotkey();
    }
    if cfg!(feature = "datapool") {
        gen_datapool();
    }

    // Note: need to specify linker flags for this after linking all the
    // other libraries since otherwise we'll get linker errors on linux.
    println!("cargo:rustc-link-lib=ccommon-2.1.0");
    println!(
        "cargo:rustc-link-search={}/ccommon/lib",
        get_cmake_binary_dir()
    );
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
