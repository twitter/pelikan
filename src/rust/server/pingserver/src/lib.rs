// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Segcache is a cache implementation which used segment based storage and uses
//! a subset of the Memcache protocol. Segment based storage allows us to
//! perform efficient eager expiration of items.

use config::PingserverConfig;
use entrystore::Noop;
use protocol::ping::{PingRequest, PingResponse};
use server::{Process, ProcessBuilder};

/// This structure represents a running `Pingserver` process.
pub struct Pingserver {
    process: Process,
}

impl Pingserver {
    /// Creates a new `Pingserver` process from the given `PingserverConfig`.
    pub fn new(config: PingserverConfig) -> Self {
        let storage = Noop::default();
        let process_builder = ProcessBuilder::<Noop, PingRequest, PingResponse>::new(
            config.admin(),
            config.server(),
            config.tls(),
            config.worker(),
            storage,
        );
        let process = process_builder.spawn();
        Self { process }
    }

    /// Wait for all threads to complete. Blocks until the process has fully
    /// terminated. Under normal conditions, this will block indefinitely.
    pub fn wait(self) {
        self.process.wait()
    }

    /// Triggers a shutdown of the process and blocks until the process has
    /// fully terminated. This is more likely to be used for running integration
    /// tests or other automated testing.
    pub fn shutdown(self) {
        self.process.shutdown()
    }
}
