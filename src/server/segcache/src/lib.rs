// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Segcache is a cache implementation which used segment based storage and uses
//! a subset of the Memcache protocol. Segment based storage allows us to
//! perform efficient eager expiration of items.

use config::*;
use entrystore::Seg;
use logger::*;
use protocol_memcache::{Request, RequestParser, Response};
use server::{Process, ProcessBuilder};
use service_memcache::MemcacheServer;

type Server = MemcacheServer;
type Parser = RequestParser;
type Storage = Seg;

/// This structure represents a running `Segcache` process.
#[allow(dead_code)]
pub struct Segcache {
    process: Process,
}

impl Segcache {
    /// Creates a new `Segcache` process from the given `SegcacheConfig`.
    pub fn new(config: SegcacheConfig) -> Result<Self, std::io::Error> {
        // initialize logging
        let log_drain = configure_logging(&config);

        // initialize metrics
        common::metrics::init();

        // initialize storage
        let storage = Storage::new(&config)?;

        let max_buffer_size = std::cmp::max(
            server::DEFAULT_BUFFER_SIZE,
            config.seg().segment_size() as usize * 2,
        );

        // initialize server service
        let server = Server::from(
            Parser::new()
                .max_value_size(config.seg().segment_size() as usize)
                .time_type(config.time().time_type()),
        );

        // initialize process
        let process_builder = ProcessBuilder::<Storage, Server, Request, Response>::new(
            config,
            storage,
            max_buffer_size,
            server,
            log_drain,
        )
        .version(env!("CARGO_PKG_VERSION"));

        // spawn threads
        let process = process_builder.spawn();

        Ok(Self { process })
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

common::metrics::test_no_duplicates!();
