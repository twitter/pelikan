// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Segcache is a cache implementation which used segment based storage and uses
//! a subset of the Memcache protocol. Segment based storage allows us to
//! perform efficient eager expiration of items.

use config::SegcacheConfig;
use entrystore::Seg;
use logger::*;
use protocol::memcache::{MemcacheRequest, MemcacheRequestParser, MemcacheResponse};
use server::{Process, ProcessBuilder};

type Parser = MemcacheRequestParser;
type Request = MemcacheRequest;
type Response = MemcacheResponse;
type Storage = Seg;

/// This structure represents a running `Segcache` process.
#[allow(dead_code)]
pub struct Segcache {
    process: Process,
}

impl Segcache {
    /// Creates a new `Segcache` process from the given `SegcacheConfig`.
    pub fn new(config: SegcacheConfig) -> Self {
        // initialize logging
        let debug_output: Box<dyn Output> = if let Some(file) = config.debug().log_file() {
            let backup = config.debug().log_backup().unwrap_or(format!("{}.old", file));
            Box::new(File::new(&file, &backup, config.debug().log_max_size()).expect("failed to open debug log file"))
        } else {
            Box::new(Stdout::new())
        };

        let debug_log = LogBuilder::new().output(debug_output).build().expect("failed to initialize debug log");

        let mut log_builder = MultiLogBuilder::new().default(debug_log);

        if let Some(file) = config.klog().file() {
            let backup = config.klog().backup().unwrap_or(format!("{}.old", file));
            let output = Box::new(File::new(&file, &backup, config.klog().max_size()).expect("failed to open klog file"));
            let klog = LogBuilder::new().output(output).build().expect("failed to initialize klog");
            log_builder = log_builder.add_target("klog", klog);
        }

        let (logger, log_drain) = log_builder.build();

        logger.start();

        // initialize metrics
        metrics::init();

        // initialize storage
        let storage = Storage::new(config.seg());

        let max_buffer_size = std::cmp::max(
            server::DEFAULT_BUFFER_SIZE,
            config.seg().segment_size() as usize * 2,
        );

        // initialize parser
        let parser = Parser::new(
            config.seg().segment_size() as usize,
            config.time().time_type(),
        );

        // initialize process
        let process_builder = ProcessBuilder::<Storage, Parser, Request, Response>::new(
            config.admin(),
            config.server(),
            config.tls(),
            config.worker(),
            storage,
            max_buffer_size,
            parser,
            log_drain,
        );

        // spawn threads
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

metrics::test_no_duplicates!();
