fn main() {
    let dst = cmake::build("../..");

    println!("cargo:rustc-link-search=native={}/lib", dst.display());
    println!("cargo:rustc-link-lib=static=ccommon-2.1.0");
    if cfg!(target_os = "macos") {
        println!("cargo:rustc-link-search=framework=/System/Library/Frameworks");
        println!("cargo:rustc-link-lib=framework=Security");
    }

    let bindings = bindgen::Builder::default()
        .clang_args(vec![
            "-I",
            "../../include",
            "-I",
            &format!("{}/build", dst.display()),
        ])
        .header("wrapper.h")
        .prepend_enum_name(false)
        .whitelist_recursively(true)
        // C stdlib types that we want to get from the libc crate
        .blacklist_type("FILE")
        .blacklist_type("addrinfo")
        .blacklist_type("timespec")
        // We provide a custom binding of metric with atomic types
        .blacklist_type("metric")
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

    bindings
        .write_to_file("src/bindings.rs")
        .expect("Couldn't write bindings!");

    let have_working_rustfmt = std::process::Command::new("rustup")
        .args(&["run", "rustfmt", "--version"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .ok()
        .map_or(false, |status| status.success());

    if !have_working_rustfmt {
        println!(
            "
        The latest `rustfmt` is required to format the generated bindings. Install
            `rustfmt` with:
            $ rustup component add rustfmt
            $ rustup update
            "
        );
    }
}
