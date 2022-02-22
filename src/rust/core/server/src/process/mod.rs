// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! This module defines the server process as a collection of threads which can
//! be spawned and provides a `Process` type which is used as a control handle
//! to shutdown or wait on the threads.

use crate::threads::*;
use common::signal::Signal;
use config::*;
use crossbeam_channel::{bounded, Sender};
use logger::Drain;
use queues::Queues;
use std::thread::JoinHandle;

const THREAD_PREFIX: &str = "pelikan";

use entrystore::EntryStore;
use protocol::{Compose, Execute, Parse};

/// A builder for Pelikan server processes.
pub struct ProcessBuilder<Storage, Parser, Request, Response>
where
    Storage: Execute<Request, Response> + EntryStore + Send,
    Parser: Parse<Request> + Clone + Send,
    Request: Send,
    Response: Compose + std::marker::Send,
{
    admin: AdminBuilder,
    listener: ListenerBuilder,
    workers: WorkersBuilder<Storage, Parser, Request, Response>,
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

        let mut admin = AdminBuilder::new(&config, ssl_context, log_drain).unwrap_or_else(|e| {
            error!("failed to initialize admin: {}", e);
            std::process::exit(1);
        });

        let workers = WorkersBuilder::new(&config, storage, parser).unwrap_or_else(|e| {
            error!("failed to initialize workers: {}", e);
            let _ = admin.log_flush();
            std::process::exit(1);
        });

        // initialize server
        let ssl_context = common::ssl::ssl_context(config.tls()).unwrap_or_else(|e| {
            error!("failed to initialize TLS: {}", e);
            let _ = admin.log_flush();
            std::process::exit(1);
        });
        let listener =
            ListenerBuilder::new(&config, ssl_context, max_buffer_size).unwrap_or_else(|e| {
                error!("failed to initialize listener: {}", e);
                let _ = admin.log_flush();
                std::process::exit(1);
            });

        Self {
            admin,
            listener,
            workers,
        }
    }

    /// Convert the `ProcessBuilder` to a running `Process` by spawning the
    /// threads for each component. Returns a `Process` which serves as a
    /// control handle for the threads.
    pub fn spawn(self) -> Process {
        let mut thread_wakers = vec![self.listener.waker()];
        thread_wakers.extend_from_slice(&self.workers.wakers());

        // channel for the parent `Process` to send `Signal`s to the admin thread
        let (signal_tx, signal_rx) = bounded(1024);

        // queues for the `Admin` to send `Signal`s to all sibling threads
        let (mut signal_queue_tx, mut signal_queue_rx) =
            Queues::new(vec![self.admin.waker()], thread_wakers);

        // queues for the `Listener` to send `Session`s to the worker threads
        let (mut session_queue_tx, session_queue_rx) =
            Queues::new(vec![self.listener.waker()], self.workers.worker_wakers());

        let mut admin = self.admin.build(signal_queue_tx.remove(0), signal_rx);
        let mut listener = self
            .listener
            .build(signal_queue_rx.remove(0), session_queue_tx.remove(0));
        let workers = self.workers.build(signal_queue_rx, session_queue_rx);

        let admin = std::thread::Builder::new()
            .name(format!("{}_admin", THREAD_PREFIX))
            .spawn(move || admin.run())
            .unwrap();

        let workers = workers.spawn();

        let listener = std::thread::Builder::new()
            .name(format!("{}_listener", THREAD_PREFIX))
            .spawn(move || listener.run())
            .unwrap();

        Process {
            admin,
            listener,
            workers,
            signal_tx,
        }
    }
}

/// This type provides a control handle for all the threads within the server
/// process.
pub struct Process {
    admin: JoinHandle<()>,
    listener: JoinHandle<()>,
    workers: Vec<JoinHandle<()>>,
    signal_tx: Sender<Signal>,
}

impl Process {
    /// Attempts to gracefully shutdown the `Process` by sending a shutdown to
    /// each thread and then waiting to join those threads.
    ///
    /// Will terminate ungracefully if it encounters an error in sending a
    /// shutdown to any of the threads.
    ///
    /// This function will block until all threads have terminated.
    pub fn shutdown(self) {
        // this sends a shutdown to the admin thread, which will broadcast the
        // signal to all sibling threads in the process
        if self.signal_tx.try_send(Signal::Shutdown).is_err() {
            fatal!("error sending shutdown signal to thread");
        }

        // wait and join all threads
        self.wait()
    }

    /// Will block until all threads terminate. This should be used to keep the
    /// process alive while the child threads run.
    pub fn wait(self) {
        for thread in self.workers {
            let _ = thread.join();
        }
        let _ = self.listener.join();
        let _ = self.admin.join();
    }
}
