// Copyright 2020 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

#[macro_use]
extern crate rustcommon_logger;

#[macro_use]
extern crate rustcommon_fastmetrics;

use config::TwemcacheConfig as Config;
use crossbeam_channel::Sender;
use mio::*;
use slab::Slab;

use std::net::SocketAddr;
use std::sync::Arc;
use std::thread::JoinHandle;

mod admin;
mod common;
mod event_loop;
mod hasher;
pub mod protocol;
mod server;
mod session;
mod storage;
mod worker;

use crate::admin::Admin;
use crate::common::Message;
use crate::hasher::CacheHasher;
use crate::server::Server;
use crate::storage::Storage;
use crate::worker::{MultiWorker, SingleWorker};

/// A structure which represents a threaded twemcache which is not yet running.
pub struct MultiWorkerTwemcacheBuilder {
    admin: Admin,
    server: Server,
    storage: Storage<CacheHasher>,
    workers: Vec<MultiWorker>,
}

pub struct SingleWorkerTwemcacheBuilder {
    admin: Admin,
    server: Server,
    worker: SingleWorker<CacheHasher>,
}

pub enum TwemcacheBuilder {
    MultiWorker(MultiWorkerTwemcacheBuilder),
    SingleWorker(SingleWorkerTwemcacheBuilder),
}

/// A structure which represents a running twemcache.
///
/// Note: for long-running daemon, be sure to call `wait()` on this structure to
/// block the process until the threads terminate. For use within tests, be sure
/// to call `shutdown()` to terminate the threads and block until termination.
pub struct Twemcache {
    admin_thread: JoinHandle<()>,
    admin_message_sender: Sender<Message>,
    server_thread: JoinHandle<()>,
    server_message_sender: Sender<Message>,
    storage_thread: Option<JoinHandle<()>>,
    storage_message_sender: Option<Sender<Message>>,
    worker_threads: Vec<JoinHandle<()>>,
    worker_message_senders: Vec<Sender<Message>>,
}

impl TwemcacheBuilder {
    /// Creates a new `TwemcacheBuilder` with an optional config file.
    ///
    /// This function will terminate the program execution if there are any
    /// issues encountered while initializing the components.
    pub fn new(config_file: Option<String>) -> Self {
        // initialize metrics
        metrics::init();

        // load config from file
        let config = if let Some(file) = config_file {
            debug!("loading config: {}", file);
            match Config::load(&file) {
                Ok(c) => Arc::new(c),
                Err(e) => {
                    error!("{}", e);
                    std::process::exit(1);
                }
            }
        } else {
            Arc::new(Default::default())
        };

        if config.worker().threads() > 1 {
            Self::multi(config)
        } else {
            Self::single(config)
        }
    }

    fn multi(config: Arc<Config>) -> Self {
        // initialize admin
        let admin = Admin::new(config.clone()).unwrap_or_else(|e| {
            error!("{}", e);
            std::process::exit(1);
        });

        // initialize storage
        let mut storage = Storage::new(config.clone()).unwrap_or_else(|e| {
            error!("{}", e);
            std::process::exit(1);
        });

        // initialize workers
        let mut workers = Vec::new();
        let mut session_senders = Vec::new();
        for _ in 0..config.worker().threads() {
            // let storage_channel = storage.add_queue();
            let worker = MultiWorker::new(config.clone(), &mut storage).unwrap_or_else(|e| {
                error!("{}", e);
                std::process::exit(1);
            });
            session_senders.push(worker.session_sender());
            workers.push(worker);
        }

        // initialize server
        let server = Server::new(config, session_senders).unwrap_or_else(|e| {
            error!("{}", e);
            std::process::exit(1);
        });

        TwemcacheBuilder::MultiWorker(MultiWorkerTwemcacheBuilder {
            admin,
            server,
            storage,
            workers,
        })
    }

    fn single(config: Arc<Config>) -> Self {
        // initialize admin
        let admin = Admin::new(config.clone()).unwrap_or_else(|e| {
            error!("{}", e);
            std::process::exit(1);
        });

        // initialize worker
        let worker = SingleWorker::new(config.clone()).unwrap_or_else(|e| {
            error!("{}", e);
            std::process::exit(1);
        });
        let session_senders = vec![worker.session_sender()];

        // initialize server
        let server = Server::new(config, session_senders).unwrap_or_else(|e| {
            error!("{}", e);
            std::process::exit(1);
        });

        TwemcacheBuilder::SingleWorker(SingleWorkerTwemcacheBuilder {
            admin,
            server,
            worker,
        })
    }

    /// Converts the `TwemcacheBuilder` to a running `Twemcache` by spawning
    /// the threads for each component. Returns a `Twemcache` which may be used
    /// to block until the threads have exited or trigger a shutdown.
    pub fn spawn(self) -> Twemcache {
        match self {
            TwemcacheBuilder::MultiWorker(b) => Self::spawn_multi(b),
            TwemcacheBuilder::SingleWorker(b) => Self::spawn_single(b),
        }
    }

    fn spawn_multi(builder: MultiWorkerTwemcacheBuilder) -> Twemcache {
        // get message senders for each component
        let admin_message_sender = builder.admin.message_sender();
        let server_message_sender = builder.server.message_sender();
        let storage_message_sender = Some(builder.storage.message_sender());
        let worker_message_senders: Vec<Sender<Message>> = builder
            .workers
            .iter()
            .map(|worker| worker.message_sender())
            .collect();
        // let worker_message_sender = self.worker.message_sender();

        // temporary bindings to prevent borrow-checker issues
        let mut admin = builder.admin;
        let mut server = builder.server;
        let mut storage = builder.storage;
        let workers = builder.workers;

        let mut worker_threads = Vec::new();

        // spawn a thread for each component
        let thread_prefix = "pelikan";
        let admin_thread = std::thread::Builder::new()
            .name(format!("{}_admin", thread_prefix))
            .spawn(move || admin.run())
            .unwrap();
        let storage_thread = Some(
            std::thread::Builder::new()
                .name(format!("{}_storage", thread_prefix))
                .spawn(move || storage.run())
                .unwrap(),
        );
        for mut worker in workers {
            let worker_thread = std::thread::Builder::new()
                .name(format!("{}_worker{}", thread_prefix, worker_threads.len()))
                .spawn(move || worker.run())
                .unwrap();
            worker_threads.push(worker_thread);
        }
        let server_thread = std::thread::Builder::new()
            .name(format!("{}_server", thread_prefix))
            .spawn(move || server.run())
            .unwrap();

        // return a `Twemcache`
        Twemcache {
            admin_thread,
            admin_message_sender,
            server_thread,
            server_message_sender,
            storage_thread,
            storage_message_sender,
            worker_threads,
            worker_message_senders,
        }
    }

    fn spawn_single(builder: SingleWorkerTwemcacheBuilder) -> Twemcache {
        // get message senders for each component
        let admin_message_sender = builder.admin.message_sender();
        let server_message_sender = builder.server.message_sender();
        let storage_message_sender = None;
        let worker_message_senders = vec![builder.worker.message_sender()];

        // temporary bindings to prevent borrow-checker issues
        let mut admin = builder.admin;
        let mut server = builder.server;
        let mut worker = builder.worker;

        // spawn a thread for each component
        let thread_prefix = "pelikan";
        let admin_thread = std::thread::Builder::new()
            .name(format!("{}_admin", thread_prefix))
            .spawn(move || admin.run())
            .unwrap();
        let storage_thread = None;
        let worker_threads = vec![std::thread::Builder::new()
            .name(format!("{}_worker", thread_prefix))
            .spawn(move || worker.run())
            .unwrap()];
        let server_thread = std::thread::Builder::new()
            .name(format!("{}_server", thread_prefix))
            .spawn(move || server.run())
            .unwrap();

        // return a `Twemcache`
        Twemcache {
            admin_thread,
            admin_message_sender,
            server_thread,
            server_message_sender,
            storage_thread,
            storage_message_sender,
            worker_threads,
            worker_message_senders,
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
    pub fn shutdown(mut self) {
        // send shutdown message to each thread
        if self.server_message_sender.send(Message::Shutdown).is_err() {
            fatal!("error sending shutdown message to server thread");
        }
        for sender in self.worker_message_senders.iter_mut() {
            if sender.send(Message::Shutdown).is_err() {
                fatal!("error sending shutdown message to worker thread");
            }
        }
        if let Some(ref sender) = self.storage_message_sender {
            if sender.send(Message::Shutdown).is_err() {
                fatal!("error sending shutdown message to admin thread");
            }
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
        for handle in self.worker_threads {
            let _ = handle.join();
        }
        if let Some(handle) = self.storage_thread {
            let _ = handle.join();
        }
        let _ = self.admin_thread.join();
    }
}
