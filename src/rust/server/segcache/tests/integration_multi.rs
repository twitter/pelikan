// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! This test module runs the integration test suite against a multi-threaded
//! instance of Segcache.

#[macro_use]
extern crate logger;

mod common;

use common::*;

use config::SegcacheConfig;
use pelikan_segcache_rs::Segcache;

use std::time::Duration;

fn main() {
    debug!("launching multi-worker server");
    let mut config = SegcacheConfig::default();
    config.worker_mut().set_threads(2);
    let server = Segcache::new(config);

    // wait for server to startup. duration is chosen to be longer than we'd
    // expect startup to take in a slow ci environment.
    std::thread::sleep(Duration::from_secs(10));

    tests();

    // shutdown server and join
    info!("shutdown...");
    let _ = server.shutdown();

    info!("passed!");
}
