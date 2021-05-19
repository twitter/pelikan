// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

#[macro_use]
extern crate rustcommon_logger;

#[macro_use]
extern crate rustcommon_fastmetrics;

mod backend;
mod common;
mod protocol;
mod session;
mod storage;
mod threads;

use backend::Backend;
pub use backend::BackendBuilder;
use config::TwemcacheConfig;
use protocol::memcache::data::{MemcacheRequest, MemcacheResponse};
use storage::SegCache;

pub struct SegcacheBackend {
    backend: Backend,
}

impl SegcacheBackend {
    pub fn new(config: TwemcacheConfig) -> Self {
        let storage: SegCache = SegCache::new(config.segcache(), config.time().time_type());
        let backend_builder = BackendBuilder::<SegCache, MemcacheRequest, MemcacheResponse>::new(
            config.admin(),
            config.server(),
            config.tls(),
            config.worker(),
            storage,
        );
        let backend = backend_builder.spawn();
        Self { backend }
    }

    pub fn wait(self) {
        self.backend.wait()
    }

    pub fn shutdown(self) {
        self.backend.shutdown()
    }
}
