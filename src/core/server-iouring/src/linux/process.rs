// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::*;
use admin::AdminBuilder;
use common::signal::Signal;
use config::*;
use crossbeam_channel::{bounded, Sender};
use entrystore::EntryStore;
use logger::Drain;
use protocol_common::*;
use queues::Queues;
use std::io::Result;
use std::thread::JoinHandle;

const QUEUE_RETRIES: usize = 3;

const QUEUE_CAPACITY: usize = 64 * 1024;

// const LISTENER_TOKEN: Token = Token(usize::MAX - 1);
// const WAKER_TOKEN: Token = Token(usize::MAX);

const THREAD_PREFIX: &str = "pelikan";

pub struct ProcessBuilder<Parser, Request, Response, Storage>
where
    Parser: 'static + Parse<Request> + Clone + Send,
    Request: 'static + Send,
    Response: 'static + Compose + Send,
    Storage: 'static + Execute<Request, Response> + EntryStore + Send,
{
    admin: AdminBuilder,
    listener: ListenerBuilder<Parser, Request, Response>,
    log_drain: Box<dyn Drain>,
    worker: WorkerBuilder<Parser, Request, Response, Storage>,
}

impl<Parser, Request, Response, Storage> ProcessBuilder<Parser, Request, Response, Storage>
where
    Parser: 'static + Parse<Request> + Clone + Send,
    Request: 'static + Send,
    Response: 'static + Compose + Send,
    Storage: 'static + Execute<Request, Response> + EntryStore + Send,
{
    pub fn new<T: AdminConfig + ServerConfig + ListenerConfig + TlsConfig + WorkerConfig>(
        config: &T,
        log_drain: Box<dyn Drain>,
        parser: Parser,
        storage: Storage,
    ) -> Result<Self> {
        let admin = AdminBuilder::new(config)?;
        let listener = ListenerBuilder::new(config, parser.clone())?;
        let worker = WorkerBuilder::new(parser, storage)?;

        Ok(Self {
            admin,
            listener,
            log_drain,
            worker,
        })
    }

    pub fn version(mut self, version: &str) -> Self {
        // self.admin.version(version);
        self
    }

    pub fn spawn(mut self) -> Process {
        let mut thread_wakers = vec![self.listener.waker()];
        // thread_wakers.extend_from_slice(&self.worker.waker());
        thread_wakers.push(self.worker.waker());

        let listener_waker = self.listener.waker();
        let worker_waker = self.worker.waker();

        // queues for the `Listener` to send `Session`s to the worker threads
        let (mut listener_session_queues, mut worker_session_queues) = Queues::new(
            vec![self.listener.waker()],
            vec![self.worker.waker()],
            QUEUE_CAPACITY,
        );

        // channel for the parent `Process` to send `Signal`s to the admin thread
        let (signal_tx, signal_rx) = bounded(QUEUE_CAPACITY);

        // queues for the `Admin` to send `Signal`s to all sibling threads
        let (mut signal_queue_tx, mut signal_queue_rx) =
            Queues::new(vec![self.admin.waker()], thread_wakers, QUEUE_CAPACITY);

        let mut admin = self
            .admin
            .build(self.log_drain, signal_rx, signal_queue_tx.remove(0));

        let mut listener = self.listener.build(listener_session_queues.pop().unwrap());

        // let worker = self.worker.build(worker_session_queues, signal_queue_rx);
        let worker = self.worker.build(worker_session_queues.pop().unwrap());

        let admin = std::thread::Builder::new()
            .name(format!("{}_admin", THREAD_PREFIX))
            .spawn(move || admin.run())
            .unwrap();

        let listener = std::thread::Builder::new()
            .name(format!("{}_listener", THREAD_PREFIX))
            .spawn(move || listener.run())
            .unwrap();

        let worker = std::thread::Builder::new()
            .name(format!("{}_worker", THREAD_PREFIX))
            .spawn(move || worker.run())
            .unwrap();

        // let mut log_drain = self.log_drain;

        // let logging = std::thread::Builder::new()
        //     .name(format!("{}_logger", THREAD_PREFIX))
        //     .spawn(move || loop { log_drain.flush(); std::thread::sleep(core::time::Duration::from_millis(1)); common::time::refresh_clock(); } )
        //     .unwrap();

        // let workers = worker.spawn();

        Process {
            admin,
            listener,
            // logging,
            signal_tx,
            worker,
        }
    }
}

pub struct Process {
    admin: JoinHandle<()>,
    listener: JoinHandle<()>,
    // logging: JoinHandle<()>,
    signal_tx: Sender<Signal>,
    worker: JoinHandle<()>,
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
        // if self.signal_tx.try_send(Signal::Shutdown).is_err() {
        //     fatal!("error sending shutdown signal to thread");
        // }

        // wait and join all threads
        self.wait()
    }

    /// Will block until all threads terminate. This should be used to keep the
    /// process alive while the child threads run.
    pub fn wait(self) {
        // for thread in self.workers {
        //     let _ = thread.join();
        // }
        let _ = self.worker.join();
        let _ = self.listener.join();
        // let _ = self.logging.join();
        let _ = self.admin.join();
    }
}
