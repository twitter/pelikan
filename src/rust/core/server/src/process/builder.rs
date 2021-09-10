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
use logger::LogReceiver;
use protocol::{Compose, Execute, Parse};
use queues::QueuePairs;

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
    pub fn new(
        admin_config: &AdminConfig,
        server_config: &ServerConfig,
        tls_config: &TlsConfig,
        worker_config: &WorkerConfig,
        storage: Storage,
        max_buffer_size: usize,
        parser: Parser,
        logger: LogReceiver,
    ) -> Self {
        // initialize admin
        let ssl_context = common::ssl::ssl_context(tls_config).unwrap_or_else(|e| {
            error!("failed to initialize TLS: {}", e);
            std::process::exit(1);
        });
        let admin = Admin::new(admin_config, ssl_context, logger).unwrap_or_else(|e| {
            error!("failed to initialize admin: {}", e);
            std::process::exit(1);
        });

        let mut worker = if worker_config.threads() > 1 {
            Self::multi_worker(worker_config, storage, parser)
        } else {
            Self::single_worker(worker_config, storage, parser)
        };

        // initialize server
        let ssl_context = common::ssl::ssl_context(tls_config).unwrap_or_else(|e| {
            error!("failed to initialize TLS: {}", e);
            std::process::exit(1);
        });
        let mut listener = Listener::new(server_config, ssl_context, max_buffer_size)
            .unwrap_or_else(|e| {
                error!("failed to initialize listener: {}", e);
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
    fn multi_worker(
        worker_config: &WorkerConfig,
        storage: Storage,
        parser: Parser,
    ) -> WorkerBuilder<Storage, Parser, Request, Response> {
        // initialize storage
        let mut storage = StorageWorker::new(worker_config, storage).unwrap_or_else(|e| {
            error!("{}", e);
            std::process::exit(1);
        });

        // initialize workers
        let mut workers = Vec::new();
        for _ in 0..worker_config.threads() {
            let worker = MultiWorker::new(worker_config, &mut storage, parser.clone())
                .unwrap_or_else(|e| {
                    error!("{}", e);
                    std::process::exit(1);
                });
            workers.push(worker);
        }

        WorkerBuilder::Multi { storage, workers }
    }

    // Creates a single-worker builder
    fn single_worker(
        worker_config: &WorkerConfig,
        storage: Storage,
        parser: Parser,
    ) -> WorkerBuilder<Storage, Parser, Request, Response> {
        // initialize worker
        let worker = SingleWorker::new(worker_config, storage, parser).unwrap_or_else(|e| {
            error!("{}", e);
            std::process::exit(1);
        });

        WorkerBuilder::Single { worker }
    }

    /// Converts the `ProcessBuilder` to a running `Process` by spawning the
    /// threads for each component. Returns a `Process` which may be used to
    /// block until the threads have exited or trigger a shutdown.
    pub fn spawn(mut self) -> Process {
        // get message senders for each component
        let mut signal_queue = QueuePairs::new(None);
        signal_queue.add_pair(self.admin.signal_queue());
        signal_queue.add_pair(self.listener.signal_queue());
        for queue in self.worker.signal_queues() {
            signal_queue.add_pair(queue);
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
