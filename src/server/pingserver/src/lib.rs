// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Pingserver is an implementation of a basic backend service which responds to
//! each `PING` with a `PONG`. This allows for testing of the core components
//! without the overheads associated with managing any state.
//!
//! This library is primarily used for automated testing. Users should prefer to
//! run the Pingserver binary provided by this crate.

use config::*;
use entrystore::Noop;
use logger::*;
use protocol_ping::{Request, RequestParser, Response};

#[cfg(feature = "io-uring")]
use server_iouring::{Process, ProcessBuilder};

#[cfg(not(feature = "io-uring"))]
use server::{Process, ProcessBuilder};

type Parser = RequestParser;
type Storage = Noop;

/// This structure represents a running `Pingserver` process.
pub struct Pingserver {
    process: Process,
}

impl Pingserver {
    /// Creates a new `Pingserver` process from the given `PingserverConfig`.
    pub fn new(config: PingserverConfig) -> Result<Self, std::io::Error> {
        // initialize logging
        let log_drain = configure_logging(&config);

        // initialize metrics
        common::metrics::init();

        // initialize storage
        let storage = Storage::new();

        // use a fixed buffer size for the pingserver
        // let max_buffer_size = server::DEFAULT_BUFFER_SIZE;

        // initialize parser
        let parser = Parser::new();

        // initialize process
        let process_builder = ProcessBuilder::<Parser, Request, Response, Storage>::new(
            &config, log_drain, parser,
            storage,
            // max_buffer_size,
            // parser,
            // log_drain,
        )?
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
