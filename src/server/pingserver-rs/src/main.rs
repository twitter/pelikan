// Copyright 2020 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

#[macro_use]
extern crate log;

use std::net::SocketAddr;
use std::sync::mpsc::*;
use std::sync::Arc;

use config::PingserverConfig;
use log::*;
use mio::*;
use slab::Slab;

mod event_loop;
mod logger;
mod server;
mod session;
mod worker;

use crate::logger::*;
use crate::server::Server;
use crate::worker::Worker;

fn main() {
    // initialize logging
    Logger::new()
        .label("pingserver")
        .level(Level::Info)
        .init()
        .expect("Failed to initialize logger");

    // load config from file
    let config = if let Some(file) = std::env::args().nth(1) {
        debug!("loading config: {}", file);
        match PingserverConfig::load(&file) {
            Ok(c) => Arc::new(c),
            Err(e) => {
                error!("{}", e);
                std::process::exit(1);
            }
        }
    } else {
        Arc::new(Default::default())
    };

    // create channel to move sessions from listener to worker
    let (sender, receiver) = sync_channel(128);

    // initialize worker
    let mut worker = Worker::new(config.clone(), receiver).unwrap_or_else(|e| {
        error!("{}", e);
        std::process::exit(1);
    });
    let waker = worker.waker();
    let worker_thread = std::thread::spawn(move || worker.run());

    // initialize server
    let mut server = Server::new(config, sender, waker).unwrap_or_else(|e| {
        error!("{}", e);
        std::process::exit(1);
    });
    let server_thread = std::thread::spawn(move || server.run());

    // join threads
    let _ = server_thread.join();
    let _ = worker_thread.join();
}
