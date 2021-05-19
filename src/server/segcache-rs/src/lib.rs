// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

#[macro_use]
extern crate rustcommon_logger;

#[macro_use]
extern crate rustcommon_fastmetrics;

mod common;
mod protocol;
mod session;
mod storage;
mod threads;

use crate::common::Sender;
use crate::common::Signal;
use crate::protocol::memcache::data::*;
use crate::protocol::Compose;
use crate::protocol::Execute;
use crate::protocol::Parse;
use crate::session::Session;
use crate::storage::SegCache;
use crate::threads::*;
use config::TwemcacheConfig;
use std::sync::Arc;
use std::thread::JoinHandle;

const THREAD_PREFIX: &str = "pelikan";

/// Wraps specialization of launching single or multi-threaded worker(s)
pub enum WorkerBuilder<Storage, Request, Response>
where
    Request: Parse,
    Response: Compose,
    Storage: Execute<Request, Response> + crate::storage::Storage,
{
    Multi {
        storage: StorageWorker<Storage, Request, Response>,
        workers: Vec<MultiWorker<Storage, Request, Response>>,
    },
    Single {
        worker: SingleWorker<Storage, Request, Response>,
    },
}

impl<Storage: 'static, Request: 'static, Response: 'static>
    WorkerBuilder<Storage, Request, Response>
where
    Request: Parse + Send,
    Response: Compose + Send,
    Storage: Execute<Request, Response> + crate::storage::Storage + Send,
{
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
    worker: WorkerBuilder<SegCache, MemcacheRequest, MemcacheResponse>,
}

impl Default for TwemcacheBuilder {
    fn default() -> Self {
        let config = Arc::new(TwemcacheConfig::default());
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
    pub fn new(config: Arc<TwemcacheConfig>) -> Self {
        // initialize metrics
        metrics::init();

        // initialize admin
        let ssl_context =
            crate::common::ssl_context(config.tls()).unwrap_or_else(|e| {
                error!("failed to initialize TLS: {}", e);
                std::process::exit(1);
            });
        let admin = Admin::new(config.admin(), ssl_context).unwrap_or_else(|e| {
            error!("failed to initialize admin: {}", e);
            std::process::exit(1);
        });

        let worker = if config.worker().threads() > 1 {
            Self::multi_worker(config.clone())
        } else {
            Self::single_worker(config.clone())
        };

        let session_senders = worker.session_senders();

        // initialize server
        let ssl_context =
            crate::common::ssl_context(config.tls()).unwrap_or_else(|e| {
                error!("failed to initialize TLS: {}", e);
                std::process::exit(1);
            });
        let server =
            Server::new(config.server(), session_senders, ssl_context).unwrap_or_else(|e| {
                error!("failed to initialize server: {}", e);
                std::process::exit(1);
            });

        TwemcacheBuilder {
            admin,
            server,
            worker,
        }
    }

    fn multi_worker(
        config: Arc<TwemcacheConfig>,
    ) -> WorkerBuilder<SegCache, MemcacheRequest, MemcacheResponse> {
        // initialize storage
        let mut storage = StorageWorker::new(
            config.worker(),
            SegCache::new(config.segcache(), config.time().time_type()),
        )
        .unwrap_or_else(|e| {
            error!("{}", e);
            std::process::exit(1);
        });

        // initialize workers
        let mut workers = Vec::new();
        let mut session_senders = Vec::new();
        for _ in 0..config.worker().threads() {
            let worker = MultiWorker::new(config.worker(), &mut storage).unwrap_or_else(|e| {
                error!("{}", e);
                std::process::exit(1);
            });
            session_senders.push(worker.session_sender());
            workers.push(worker);
        }

        WorkerBuilder::Multi { storage, workers }
    }

    fn single_worker(
        config: Arc<TwemcacheConfig>,
    ) -> WorkerBuilder<SegCache, MemcacheRequest, MemcacheResponse> {
        // initialize storage
        let storage = SegCache::new(config.segcache(), config.time().time_type());

        // initialize worker
        let worker = SingleWorker::new(config.worker(), storage).unwrap_or_else(|e| {
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
