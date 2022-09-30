// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use config::*;
use entrystore::Bloom;
use logger::*;
use protocol_http::{request::ParseData, RequestParser, Response};
use server::{Process, ProcessBuilder};

type Parser = RequestParser;
type Storage = Bloom;

pub struct BloomCache {
    process: Process,
}

impl BloomCache {
    /// Create a new bloom cache from the provided config.
    pub fn new(config: BloomcacheConfig) -> std::io::Result<Self> {
        common::metrics::init();

        let log = configure_logging(&config);
        let storage = Storage::new(&config)?;
        let parser = Parser::new();

        let builder = ProcessBuilder::<Parser, ParseData, Response, Storage>::new(
            &config, log, parser, storage,
        )?
        .version(env!("CARGO_PKG_VERSION"));

        Ok(Self {
            process: builder.spawn(),
        })
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
