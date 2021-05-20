// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use config::TwemcacheConfig;
use entrystore::Seg;
use protocol::memcache::{MemcacheRequest, MemcacheResponse};
use server::{Process, ProcessBuilder};

pub struct Segcache {
    process: Process,
}

impl Segcache {
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

    pub fn wait(self) {
        self.process.wait()
    }

    pub fn shutdown(self) {
        self.process.shutdown()
    }
}
