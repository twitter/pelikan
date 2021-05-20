// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use server::{Process, ProcessBuilder};
use config::TwemcacheConfig;
use protocol::memcache::data::{MemcacheRequest, MemcacheResponse};

pub struct Segcache {
    process: Process,
}

impl Segcache {
    pub fn new(config: TwemcacheConfig) -> Self {
        let storage = storage::SegCache::new(config.segcache(), config.time().time_type());
        let process_builder = ProcessBuilder::<storage::SegCache, MemcacheRequest, MemcacheResponse>::new(
            config.admin(),
            config.server(),
            config.tls(),
            config.worker(),
            storage,
        );
        let process = process_builder.spawn();
        Self { process }
    }

    pub fn wait(self) {
        self.process.wait()
    }

    pub fn shutdown(self) {
        self.process.shutdown()
    }
}
