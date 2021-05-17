// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

#[macro_use]
extern crate rustcommon_logger;

#[macro_use]
extern crate rustcommon_fastmetrics;

use crate::storage::SegCacheStorage;
use crate::protocol::data::MemcacheResponse;
use crate::protocol::data::MemcacheRequest;
use crate::common::Sender;
use crate::common::Queue;
use config::TwemcacheConfig as Config;
use mio::*;
use slab::Slab;

use std::net::SocketAddr;
use std::sync::Arc;
use std::thread::JoinHandle;

mod admin;
mod common;
mod event_loop;
pub mod protocol;
mod server;
mod session;
mod storage;
mod worker;

use crate::admin::Admin;
use crate::common::Signal;
use crate::server::Server;
use crate::session::Session;
use crate::storage::StorageWorker;
use crate::worker::{MultiWorker, SingleWorker};

const THREAD_PREFIX: &str = "pelikan";

/// Wraps specialization of launching single or multi-threaded worker(s)
pub enum WorkerBuilder {
    Multi {
        storage: StorageWorker<SegCacheStorage, MemcacheResponse, MemcacheRequest>,
        workers: Vec<MultiWorker<MemcacheRequest, MemcacheResponse>>,
    },
    Single {
        worker: SingleWorker,
    },
}

impl WorkerBuilder {
    fn session_senders(&self) -> Vec<Sender<Session>> {
        match self {
            Self::Single { worker } => {
                vec![worker.session_sender()]
            }
            Self::Multi { workers, .. } => workers.iter().map(|w| w.session_sender()).collect(),
        }
    }

    fn signal_senders(&self) -> Vec<Sender<Signal>> {
        let mut senders = Vec::new();
        match self {
            Self::Single { worker } => {
                senders.push(worker.signal_sender());
            }
            Self::Multi { storage, workers } => {
                for worker in workers {
                    senders.push(worker.signal_sender());
                }
                senders.push(storage.signal_sender());
            }
        }
        senders
    }

    fn launch_threads(self) -> Vec<JoinHandle<()>> {
        match self {
            Self::Single { mut worker } => {
                vec![std::thread::Builder::new()
                    .name(format!("{}_worker", THREAD_PREFIX))
                    .spawn(move || worker.run())
                    .unwrap()]
            }
            Self::Multi {
                mut storage,
                workers,
            } => {
                let mut threads = Vec::new();
                for mut worker in workers {
                    let worker_thread = std::thread::Builder::new()
                        .name(format!("{}_worker{}", THREAD_PREFIX, threads.len()))
                        .spawn(move || worker.run())
                        .unwrap();
                    threads.push(worker_thread);
                }
                threads.push(
                    std::thread::Builder::new()
                        .name(format!("{}_storage", THREAD_PREFIX))
                        .spawn(move || storage.run())
                        .unwrap(),
                );
                threads
            }
        }
    }
}

/// A structure which represents a twemcache instance which is not yet running.
pub struct TwemcacheBuilder {
    admin: Admin,
    server: Server,
    worker: WorkerBuilder,
}

impl Default for TwemcacheBuilder {
    fn default() -> Self {
        let config = Arc::new(Config::default());
        Self::new(config)
    }
}

/// A structure which represents a running twemcache.
///
/// Note: for long-running daemon, be sure to call `wait()` on this structure to
/// block the process until the threads terminate. For use within tests, be sure
/// to call `shutdown()` to terminate the threads and block until termination.
pub struct Twemcache {
    threads: Vec<JoinHandle<()>>,
    signal_senders: Vec<Sender<Signal>>,
}

impl TwemcacheBuilder {
    /// Creates a new `TwemcacheBuilder` with an optional config file.
    ///
    /// This function will terminate the program execution if there are any
    /// issues encountered while initializing the components.
    pub fn new(config: Arc<Config>) -> Self {
        // initialize metrics
        metrics::init();

        // initialize admin
        let admin = Admin::new(config.clone()).unwrap_or_else(|e| {
            error!("{}", e);
            std::process::exit(1);
        });

        let worker = if config.worker().threads() > 1 {
            Self::multi_worker(config.clone())
        } else {
            Self::single_worker(config.clone())
        };

        let session_senders = worker.session_senders();

        // initialize server
        let server = Server::new(config, session_senders).unwrap_or_else(|e| {
            error!("{}", e);
            std::process::exit(1);
        });

        TwemcacheBuilder {
            admin,
            server,
            worker,
        }
    }

    fn multi_worker(config: Arc<Config>) -> WorkerBuilder {
        // initialize storage
        let mut storage = StorageWorker::new(config.clone()).unwrap_or_else(|e| {
            error!("{}", e);
            std::process::exit(1);
        });

        // initialize workers
        let mut workers = Vec::new();
        let mut session_senders = Vec::new();
        for _ in 0..config.worker().threads() {
            let worker = MultiWorker::new(config.clone(), &mut storage).unwrap_or_else(|e| {
                error!("{}", e);
                std::process::exit(1);
            });
            session_senders.push(worker.session_sender());
            workers.push(worker);
        }

        WorkerBuilder::Multi { storage, workers }
    }

    fn single_worker(config: Arc<Config>) -> WorkerBuilder {
        // initialize worker
        let worker = SingleWorker::new(config).unwrap_or_else(|e| {
            error!("{}", e);
            std::process::exit(1);
        });

        WorkerBuilder::Single { worker }
    }

    /// Converts the `TwemcacheBuilder` to a running `Twemcache` by spawning
    /// the threads for each component. Returns a `Twemcache` which may be used
    /// to block until the threads have exited or trigger a shutdown.
    pub fn spawn(self) -> Twemcache {
        // get message senders for each component
        let mut signal_senders = vec![self.server.signal_sender()];
        signal_senders.extend_from_slice(&self.worker.signal_senders());
        signal_senders.push(self.admin.signal_sender());

        // temporary bindings to prevent borrow-checker issues
        let mut admin = self.admin;
        let mut server = self.server;

        // spawn a thread for each component
        let mut threads = vec![std::thread::Builder::new()
            .name(format!("{}_server", THREAD_PREFIX))
            .spawn(move || server.run())
            .unwrap()];
        let worker_threads = self.worker.launch_threads();
        for thread in worker_threads {
            threads.push(thread);
        }
        threads.push(
            std::thread::Builder::new()
                .name(format!("{}_admin", THREAD_PREFIX))
                .spawn(move || admin.run())
                .unwrap(),
        );

        // return a `Twemcache`
        Twemcache {
            threads,
            signal_senders,
        }
    }
}

impl Twemcache {
    /// Attempts to gracefully shutdown the `Twemcache` by sending a shutdown
    /// to each thread and then waiting to join those threads.
    ///
    /// Will terminate ungracefully if it encounters an error in sending a
    /// shutdown to any of the threads.
    ///
    /// This function will block until all threads have terminated.
    pub fn shutdown(self) {
        for sender in &self.signal_senders {
            if sender.send(Signal::Shutdown).is_err() {
                fatal!("error sending shutdown signal to thread");
            }
        }

        // wait and join all threads
        self.wait()
    }

    /// Will block until all threads terminate. This should be used to keep the
    /// process alive while the child threads run.
    pub fn wait(self) {
        for thread in self.threads {
            let _ = thread.join();
        }
    }
}
