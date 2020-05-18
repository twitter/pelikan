// Copyright 2020 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use config::PingserverConfig;
use std::net::SocketAddr;
use std::sync::mpsc::*;
use std::sync::Arc;

use log::*;
use mio::net::*;
use mio::unix::*;
use mio::*;
use slab::Slab;

mod buffer;
mod listener;
mod logger;
mod session;
mod worker;

use crate::listener::Listener;
use crate::logger::*;
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
    let (sender, receiver) = channel();

    // initialize worker
    let mut worker = Worker::new(config.clone(), receiver).unwrap_or_else(|e| {
        error!("{}", e);
        std::process::exit(1);
    });
    let worker_thread = std::thread::spawn(move || worker.run());

    // initialize listener
    let mut listener = Listener::new(config, sender).unwrap_or_else(|e| {
        error!("{}", e);
        std::process::exit(1);
    });
    let listener_thread = std::thread::spawn(move || listener.run());

    // join threads
    let _ = listener_thread.join();
    let _ = worker_thread.join();
}
