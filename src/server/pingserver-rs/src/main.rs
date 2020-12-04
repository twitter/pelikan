// Copyright 2020 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use pelikan_pingserver_rs::PingserverBuilder;

use rustcommon_logger::{Level, Logger};

fn main() {
    // initialize logging
    Logger::new()
        .label("pingserver")
        .level(Level::Info)
        .init()
        .expect("Failed to initialize logger");

    // launch pingserver
    PingserverBuilder::new(std::env::args().nth(1))
        .spawn()
        .wait()
}
