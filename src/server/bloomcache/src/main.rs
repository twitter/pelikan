// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

#[macro_use]
extern crate logger;

use backtrace::Backtrace;
use config::PingserverConfig;
use rustcommon_metrics::*;
use server::PERCENTILES;

fn main() {
    // custom panic hook to terminate whole process after unwinding
    std::panic::set_hook(Box::new(|s| {
        error!("{}", s);
        eprintln!("{:?}", Backtrace::new());
        std::process::exit(101);
    }));
}
