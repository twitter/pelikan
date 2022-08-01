// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use config::PingproxyConfig;
use logger::configure_logging;
use protocol_thrift::*;
use proxy::{Process, ProcessBuilder};

const MAX_SIZE: usize = 16 * 1024 * 1024; // 16MB

type BackendParser = MessageParser;
type BackendRequest = Message;
type BackendResponse = Message;

type FrontendParser = MessageParser;
type FrontendRequest = Message;
type FrontendResponse = Message;

#[allow(dead_code)]
pub struct Thriftproxy {
    process: Process,
}

impl Default for Thriftproxy {
    fn default() -> Self {
        Self::new(PingproxyConfig::default())
    }
}

impl Thriftproxy {
    /// Creates a new `Pingproxy` process from the given `PingproxyConfig`.
    pub fn new(config: PingproxyConfig) -> Self {
        // initialize logging
        let log_drain = configure_logging(&config);

        // initialize metrics
        common::metrics::init();

        // initialize parsers
        let request_parser = MessageParser::new(MAX_SIZE);
        let response_parser = MessageParser::new(MAX_SIZE);

       // initialize process
        let process_builder =
            ProcessBuilder::<BackendParser, BackendRequest, BackendResponse, FrontendParser, FrontendRequest, FrontendResponse>::new(&config, log_drain, response_parser, request_parser)
                .expect("failed to launch");
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

common::metrics::test_no_duplicates!();
