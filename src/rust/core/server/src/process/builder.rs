// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use super::*;
use crate::threads::*;
use crate::THREAD_PREFIX;
use config::AdminConfig;
use config::ServerConfig;
use config::TlsConfig;
use config::WorkerConfig;
use entrystore::EntryStore;
use logger::*;
use protocol_common::{Compose, Execute, Parse};
use queues::QueuePairs;
use std::io::Error;

/// A builder type for a Pelikan cache process.
pub struct ProcessBuilder<Storage, Parser, Request, Response>
where
    Storage: Execute<Request, Response> + EntryStore + Send,
    Parser: Parse<Request> + Clone + Send,
    Request: Send,
    Response: Compose + std::marker::Send,
{
    admin: Admin,
    listener: Listener,
    worker: WorkerBuilder<Storage, Parser, Request, Response>,
}

impl<Storage: 'static, Parser: 'static, Request: 'static, Response: 'static>
    ProcessBuilder<Storage, Parser, Request, Response>
where
    Storage: Execute<Request, Response> + EntryStore + Send,
    Parser: Parse<Request> + Clone + Send,
    Request: Send,
    Response: Compose + std::marker::Send,
{
    /// Creates a new `ProcessBuilder`
    ///
    /// This function will terminate the program execution if there are any
    /// issues encountered while initializing the components.
    pub fn new<T: AdminConfig + ServerConfig + WorkerConfig + TlsConfig>(
        config: T,
        storage: Storage,
        max_buffer_size: usize,
        parser: Parser,
        mut log_drain: Box<dyn Drain>,
    ) -> Self {
        // initialize admin
        let ssl_context = common::ssl::ssl_context(config.tls()).unwrap_or_else(|e| {
            error!("failed to initialize TLS: {}", e);
            let _ = log_drain.flush();
            std::process::exit(1);
        });

        let mut admin = Admin::new(&config, ssl_context, log_drain).unwrap_or_else(|e| {
            error!("failed to initialize admin: {}", e);
            std::process::exit(1);
        });

        let mut worker = if config.worker().threads() > 1 {
            Self::multi_worker(&config, storage, parser).unwrap_or_else(|e| {
                error!("failed to initialize workers: {}", e);
                let _ = admin.log_flush();
                std::process::exit(1);
            })
        } else {
            Self::single_worker(&config, storage, parser).unwrap_or_else(|e| {
                error!("failed to initialize workers: {}", e);
                let _ = admin.log_flush();
                std::process::exit(1);
            })
        };

        // initialize server
        let ssl_context = common::ssl::ssl_context(config.tls()).unwrap_or_else(|e| {
            error!("failed to initialize TLS: {}", e);
            let _ = admin.log_flush();
            std::process::exit(1);
        });
        let mut listener =
            Listener::new(&config, ssl_context, max_buffer_size).unwrap_or_else(|e| {
                error!("failed to initialize listener: {}", e);
                let _ = admin.log_flush();
                std::process::exit(1);
            });
        let mut session_queues = worker.session_queues(listener.waker());
        for session_queue in session_queues.drain(..) {
            listener.add_session_queue(session_queue);
        }

        Self {
            admin,
            listener,
            worker,
        }
    }

    // Creates a multi-worker builder
    fn multi_worker<T: WorkerConfig>(
        config: &T,
        storage: Storage,
        parser: Parser,
    ) -> Result<WorkerBuilder<Storage, Parser, Request, Response>, Error> {
        let worker_config = config.worker();

        // initialize storage
        let mut storage = StorageWorker::new(config, storage).unwrap_or_else(|e| {
            error!("{}", e);
            std::process::exit(1);
        });

        // initialize workers
        let mut workers = Vec::new();
        for _ in 0..worker_config.threads() {
            let worker =
                MultiWorker::new(config, &mut storage, parser.clone()).unwrap_or_else(|e| {
                    error!("{}", e);
                    std::process::exit(1);
                });
            workers.push(worker);
        }

        Ok(WorkerBuilder::Multi { storage, workers })
    }

    // Creates a single-worker builder
    fn single_worker<T: WorkerConfig>(
        config: &T,
        storage: Storage,
        parser: Parser,
    ) -> Result<WorkerBuilder<Storage, Parser, Request, Response>, Error> {
        // initialize worker
        let worker = SingleWorker::new(config, storage, parser).unwrap_or_else(|e| {
            error!("{}", e);
            std::process::exit(1);
        });

        Ok(WorkerBuilder::Single { worker })
    }

    /// Converts the `ProcessBuilder` to a running `Process` by spawning the
    /// threads for each component. Returns a `Process` which may be used to
    /// block until the threads have exited or trigger a shutdown.
    pub fn spawn(mut self) -> Process {
        // initialize a queue to send a signal to the admin thread
        let mut signal_queue = QueuePairs::new(None);
        signal_queue.add_pair(self.admin.signal_queue());

        // register signal queues with admin thread so it can send signals to
        // all other threads
        self.admin.add_signal_queue(self.listener.signal_queue());
        signal_queue.add_pair(self.listener.signal_queue());
        for queue in self.worker.signal_queues() {
            self.admin.add_signal_queue(queue);
        }

        // temporary bindings to prevent borrow-checker issues
        let mut admin = self.admin;
        let mut listener = self.listener;

        // spawn a thread for each component
        let mut threads = vec![std::thread::Builder::new()
            .name(format!("{}_listener", THREAD_PREFIX))
            .spawn(move || listener.run())
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

        Process {
            threads,
            signal_queue,
        }
    }
}
