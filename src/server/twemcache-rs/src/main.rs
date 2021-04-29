// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

#[macro_use]
extern crate rustcommon_logger;

use backtrace::Backtrace;
use pelikan_twemcache_rs::TwemcacheBuilder;

use rustcommon_logger::{Level, Logger};

fn main() {
    // custom panic hook to terminate whole process after unwinding
    std::panic::set_hook(Box::new(|s| {
        error!("{}", s);
        println!("{:?}", Backtrace::new());
        std::process::exit(101);
    }));

    // initialize logging
    Logger::new()
        .label(env!("CARGO_CRATE_NAME"))
        .level(Level::Info)
        .init()
        .expect("Failed to initialize logger");

    // launch twemcache
    TwemcacheBuilder::new(std::env::args().nth(1))
        .spawn()
        .wait()
}
