// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Segcache is a cache implementation which used segment based storage and uses
//! a subset of the Memcache protocol. Segment based storage allows us to
//! perform efficient eager expiration of items.

use config::TwemcacheConfig;
use entrystore::Seg;
use protocol::memcache::{MemcacheRequest, MemcacheResponse};
use server::{Process, ProcessBuilder};

/// This structure represents a running `Segcache` process.
pub struct Segcache {
    process: Process,
}

impl Segcache {
    /// Creates a new `Segcache` process from the given `TwemcacheConfig`.
    pub fn new(config: TwemcacheConfig) -> Self {
        let storage = Seg::new(config.seg(), config.time().time_type());
        let process_builder = ProcessBuilder::<Seg, MemcacheRequest, MemcacheResponse>::new(
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
