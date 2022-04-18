// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! This test module runs the integration test suite against a single-threaded
//! instance of Segcache.

mod common;

#[macro_use]
extern crate logger;

use common::*;

use config::SegcacheConfig;
use pelikan_segcache_rs::Segcache;

use std::time::Duration;

fn main() {
    debug!("launching server");
    let server = Segcache::new(SegcacheConfig::default());

    // wait for server to startup. duration is chosen to be longer than we'd
    // expect startup to take in a slow ci environment.
    std::thread::sleep(Duration::from_secs(10));

    tests();

    admin_tests();

    // shutdown server and join
    info!("shutdown...");
    let _ = server.shutdown();

    info!("passed!");
}
