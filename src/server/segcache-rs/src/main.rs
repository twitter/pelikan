// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

#[macro_use]
extern crate rustcommon_logger;

use backtrace::Backtrace;
use std::sync::Arc;

use config::TwemcacheConfig;
use pelikan_segcache_rs::TwemcacheBuilder;

use rustcommon_logger::Logger;

fn main() {
    // custom panic hook to terminate whole process after unwinding
    std::panic::set_hook(Box::new(|s| {
        error!("{}", s);
        println!("{:?}", Backtrace::new());
        std::process::exit(101);
    }));

    // load config from file
    let config = if let Some(file) = std::env::args().nth(1) {
        debug!("loading config: {}", file);
        match TwemcacheConfig::load(&file) {
            Ok(c) => Arc::new(c),
            Err(e) => {
                error!("{}", e);
                std::process::exit(1);
            }
        }
    } else {
        Arc::new(Default::default())
    };

    // initialize logging
    Logger::new()
        .label(env!("CARGO_CRATE_NAME"))
        .level(config.debug().log_level())
        .init()
        .expect("Failed to initialize logger");

    // launch twemcache
    TwemcacheBuilder::new(config).spawn().wait()
}
