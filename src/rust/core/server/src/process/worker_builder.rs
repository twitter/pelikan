// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::threads::*;
use crate::THREAD_PREFIX;
use common::signal::Signal;
use entrystore::EntryStore;
use mio::Waker;
use protocol_common::{Compose, Execute, Parse};
use queues::QueuePair;
use session::Session;
use std::sync::Arc;
use std::thread::JoinHandle;

/// A builder for worker threads which abstracts the differences between single
/// and multi worker processes.
pub enum WorkerBuilder<Storage, Parser, Request, Response>
where
    Parser: Parse<Request>,
    Response: Compose,
    Storage: Execute<Request, Response> + EntryStore,
{
    /// Used to create two or more `worker` threads in addition to a shared
    /// `storage` thread.
    Multi {
        storage: StorageWorker<Storage, Request, Response>,
        workers: Vec<MultiWorker<Storage, Parser, Request, Response>>,
    },
    /// Used to create a single `worker` thread with thread-local storage.
    Single {
        worker: SingleWorker<Storage, Parser, Request, Response>,
    },
}

impl<Storage: 'static, Parser: 'static, Request: 'static, Response: 'static>
    WorkerBuilder<Storage, Parser, Request, Response>
where
    Parser: Parse<Request> + Send,
    Request: Send,
    Response: Compose + Send,
    Storage: Execute<Request, Response> + EntryStore + Send,
{
    /// Return the session queues for all the workers.
    pub fn session_queues(&mut self, waker: Arc<Waker>) -> Vec<QueuePair<Session, ()>> {
        match self {
            Self::Single { worker } => {
                vec![worker.session_sender(waker)]
            }
            Self::Multi { workers, .. } => workers
                .iter_mut()
                .map(|w| w.session_sender(waker.clone()))
                .collect(),
        }
    }

    /// Return the signal queues for all the workers.
    pub fn signal_queues(&mut self) -> Vec<QueuePair<Signal, Signal>> {
        let mut senders = Vec::new();
        match self {
            Self::Single { worker } => {
                senders.push(worker.signal_queue());
            }
            Self::Multi { storage, workers } => {
                for worker in workers {
                    senders.push(worker.signal_queue());
                }
                senders.push(storage.signal_queue());
            }
        }
        senders
    }

    /// Launch all the threads.
    pub fn launch_threads(self) -> Vec<JoinHandle<()>> {
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
