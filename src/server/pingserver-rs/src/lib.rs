// Copyright 2020 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

#[macro_use]
extern crate rustcommon_logger;

use std::net::SocketAddr;
use std::sync::mpsc::*;
use std::sync::Arc;
use std::thread::JoinHandle;

use config::PingserverConfig;
use mio::*;
use rustcommon_metrics::*;
use slab::Slab;

mod admin;
mod common;
mod event_loop;
mod metrics;
mod server;
mod session;
mod worker;

use crate::admin::Admin;
use crate::common::Message;
use crate::metrics::Stat;
use crate::server::Server;
use crate::worker::Worker;

/// A structure which represents a pingserver which is not yet running.
pub struct PingserverBuilder {
    admin: Admin,
    server: Server,
    worker: Worker,
}

/// A structure which represents a running pingserver.
///
/// Note: for long-running daemon, be sure to call `wait()` on this structure to
/// block the process until the threads terminate. For use within tests, be sure
/// to call `shutdown()` to terminate the threads and block until termination.
pub struct Pingserver {
    admin_thread: JoinHandle<()>,
    admin_message_sender: SyncSender<Message>,
    server_thread: JoinHandle<()>,
    server_message_sender: SyncSender<Message>,
    worker_thread: JoinHandle<()>,
    worker_message_sender: SyncSender<Message>,
}

impl PingserverBuilder {
    /// Creates a new `PingserverBuilder` with an optional config file.
    ///
    /// This function will terminate the program execution if there are any
    /// issues encountered while initializing the components.
    pub fn new(config_file: Option<String>) -> Self {
        // initialize metrics
        let metrics = crate::metrics::init();

        // load config from file
        let config = if let Some(file) = config_file {
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

        // initialize admin
        let admin = Admin::new(config.clone(), metrics.clone()).unwrap_or_else(|e| {
            error!("{}", e);
            std::process::exit(1);
        });

        // initialize worker
        let worker = Worker::new(config.clone(), metrics.clone()).unwrap_or_else(|e| {
            error!("{}", e);
            std::process::exit(1);
        });

        // initialize server
        let server = Server::new(config, metrics, worker.session_sender()).unwrap_or_else(|e| {
            error!("{}", e);
            std::process::exit(1);
        });

        Self {
            admin,
            server,
            worker,
        }
    }

    /// Converts the `PingserverBuilder` to a running `Pingserver` by spawning
    /// the threads for each component. Returns a `Pingserver` which may be used
    /// to block until the threads have exited or trigger a shutdown.
    pub fn spawn(self) -> Pingserver {
        // get message senders for each component
        let admin_message_sender = self.admin.message_sender();
        let server_message_sender = self.server.message_sender();
        let worker_message_sender = self.worker.message_sender();

        // temporary bindings to prevent borrow-checker issues
        let mut admin = self.admin;
        let mut server = self.server;
        let mut worker = self.worker;

        // spawn a thread for each component
        let admin_thread = std::thread::spawn(move || admin.run());
        let worker_thread = std::thread::spawn(move || worker.run());
        let server_thread = std::thread::spawn(move || server.run());

        // return a `Pingserver`
        Pingserver {
            admin_thread,
            admin_message_sender,
            server_thread,
            server_message_sender,
            worker_thread,
            worker_message_sender,
        }
    }
}

impl Pingserver {
    /// Attempts to gracefully shutdown the `Pingserver` by sending a shutdown
    /// to each thread and then waiting to join those threads.
    ///
    /// Will terminate ungracefully if it encounters an error in sending a
    /// shutdown to any of the threads.
    ///
    /// This function will block until all threads have terminated.
    pub fn shutdown(self) {
        // send shutdown message to each thread
        if self.server_message_sender.send(Message::Shutdown).is_err() {
            fatal!("error sending shutdown message to server thread");
        }
        if self.worker_message_sender.send(Message::Shutdown).is_err() {
            fatal!("error sending shutdown message to worker thread");
        }
        if self.admin_message_sender.send(Message::Shutdown).is_err() {
            fatal!("error sending shutdown message to admin thread");
        }

        // wait and join all threads
        self.wait()
    }

    /// Will block until all threads terminate. This should be used to keep the
    /// process alive while the child threads run.
    pub fn wait(self) {
        // join threads
        let _ = self.server_thread.join();
        let _ = self.worker_thread.join();
        let _ = self.admin_thread.join();
    }
}
