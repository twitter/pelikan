// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use super::*;
use crate::threads::*;
use config::AdminConfig;
use config::ServerConfig;
use config::TlsConfig;
use config::WorkerConfig;
use protocol::{Compose, Execute, Parse};

const THREAD_PREFIX: &str = "pelikan";

/// A structure which represents a twemcache instance which is not yet running.
pub struct ProcessBuilder<Storage, Request, Response>
where
    Storage: Execute<Request, Response> + storage::Storage + Send,
    Request: Parse + std::marker::Send,
    Response: Compose + std::marker::Send,
{
    admin: Admin,
    listener: Listener,
    worker: WorkerBuilder<Storage, Request, Response>,
}

impl<Storage: 'static, Request: 'static, Response: 'static>
    ProcessBuilder<Storage, Request, Response>
where
    Storage: Execute<Request, Response> + storage::Storage + Send,
    Request: Parse + std::marker::Send,
    Response: Compose + std::marker::Send,
{
    /// Creates a new `TwemcacheBuilder` with an optional config file.
    ///
    /// This function will terminate the program execution if there are any
    /// issues encountered while initializing the components.
    pub fn new(
        admin_config: &AdminConfig,
        server_config: &ServerConfig,
        tls_config: &TlsConfig,
        worker_config: &WorkerConfig,
        storage: Storage,
    ) -> Self {
        // initialize metrics
        metrics::init();

        // initialize admin
        let ssl_context = common::ssl::ssl_context(tls_config).unwrap_or_else(|e| {
            error!("failed to initialize TLS: {}", e);
            std::process::exit(1);
        });
        let admin = Admin::new(admin_config, ssl_context).unwrap_or_else(|e| {
            error!("failed to initialize admin: {}", e);
            std::process::exit(1);
        });

        let worker = if worker_config.threads() > 1 {
            Self::multi_worker(worker_config, storage)
        } else {
            Self::single_worker(worker_config, storage)
        };

        let session_senders = worker.session_senders();

        // initialize server
        let ssl_context = common::ssl::ssl_context(tls_config).unwrap_or_else(|e| {
            error!("failed to initialize TLS: {}", e);
            std::process::exit(1);
        });
        let listener =
            Listener::new(server_config, session_senders, ssl_context).unwrap_or_else(|e| {
                error!("failed to initialize listener: {}", e);
                std::process::exit(1);
            });

        Self {
            admin,
            listener,
            worker,
        }
    }

    fn multi_worker(
        worker_config: &WorkerConfig,
        storage: Storage,
    ) -> WorkerBuilder<Storage, Request, Response> {
        // initialize storage
        let mut storage = StorageWorker::new(worker_config, storage).unwrap_or_else(|e| {
            error!("{}", e);
            std::process::exit(1);
        });

        // initialize workers
        let mut workers = Vec::new();
        let mut session_senders = Vec::new();
        for _ in 0..worker_config.threads() {
            let worker = MultiWorker::new(worker_config, &mut storage).unwrap_or_else(|e| {
                error!("{}", e);
                std::process::exit(1);
            });
            session_senders.push(worker.session_sender());
            workers.push(worker);
        }

        WorkerBuilder::Multi { storage, workers }
    }

    fn single_worker(
        worker_config: &WorkerConfig,
        storage: Storage,
    ) -> WorkerBuilder<Storage, Request, Response> {
        // initialize worker
        let worker = SingleWorker::new(worker_config, storage).unwrap_or_else(|e| {
            error!("{}", e);
            std::process::exit(1);
        });

        WorkerBuilder::Single { worker }
    }

    /// Converts the `TwemcacheBuilder` to a running `Twemcache` by spawning
    /// the threads for each component. Returns a `Twemcache` which may be used
    /// to block until the threads have exited or trigger a shutdown.
    pub fn spawn(self) -> Process {
        // get message senders for each component
        let mut signal_senders = vec![self.listener.signal_sender()];
        signal_senders.extend_from_slice(&self.worker.signal_senders());
        signal_senders.push(self.admin.signal_sender());

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

        // return a `Twemcache`
        Process {
            threads,
            signal_senders,
        }
    }
}
