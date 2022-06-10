// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Worker threads which are used in multi or single worker mode to handle
//! sending and receiving data on established client sessions

mod multi;
mod single;
mod storage;

use protocol_common::ExecutionResult;
pub use self::storage::{StorageWorker, StorageWorkerBuilder};
use crate::*;
use crate::{QUEUE_CAPACITY, THREAD_PREFIX};
use common::signal::Signal;
use config::WorkerConfig;
use entrystore::EntryStore;
use mio::Waker;
pub use multi::{MultiWorker, MultiWorkerBuilder};
use protocol_common::{Compose, Execute, Parse};
use queues::Queues;
use session::Session;
pub use single::{SingleWorker, SingleWorkerBuilder};
use std::io::Error;
use std::sync::Arc;
use std::thread::JoinHandle;

use super::EventLoop;
use mio::Token;

counter!(WORKER_EVENT_LOOP);
counter!(WORKER_EVENT_TOTAL);
counter!(WORKER_EVENT_ERROR);
counter!(WORKER_EVENT_WRITE);
counter!(WORKER_EVENT_READ);

counter!(STORAGE_EVENT_LOOP);
heatmap!(STORAGE_QUEUE_DEPTH, 1_000_000);

counter!(PROCESS_REQ);

type WrappedResult<Request, Response> = TokenWrapper<Box<dyn ExecutionResult<Request, Response>>>;

pub struct TokenWrapper<T> {
    inner: T,
    token: Token,
}

impl<T> TokenWrapper<T> {
    pub fn new(inner: T, token: Token) -> Self {
        Self { inner, token }
    }

    pub fn token(&self) -> Token {
        self.token
    }

    pub fn into_inner(self) -> T {
        self.inner
    }
}

/// A builder type for the worker threads which process requests and write
/// responses.
pub enum WorkersBuilder<Storage, Parser, Request, Response>
where
    Parser: Parse<Request>,
    Response: Compose,
    Storage: Execute<Request, Response> + EntryStore,
{
    /// Used to create two or more `worker` threads in addition to a shared
    /// `storage` thread.
    Multi {
        storage: StorageWorkerBuilder<Storage, Request, Response>,
        workers: Vec<MultiWorkerBuilder<Storage, Parser, Request, Response>>,
    },
    /// Used to create a single `worker` thread with thread-local storage.
    Single {
        worker: SingleWorkerBuilder<Storage, Parser, Request, Response>,
    },
}

impl<Storage, Parser, Request, Response> WorkersBuilder<Storage, Parser, Request, Response>
where
    Parser: Parse<Request> + Clone,
    Response: Compose,
    Storage: Execute<Request, Response> + EntryStore,
{
    /// Create a new `WorkersBuilder` from the provided config, storage, and
    /// parser.
    pub fn new<T: WorkerConfig>(
        config: &T,
        storage: Storage,
        parser: Parser,
    ) -> Result<WorkersBuilder<Storage, Parser, Request, Response>, Error> {
        let worker_config = config.worker();

        if worker_config.threads() == 1 {
            Self::single_worker(config, storage, parser)
        } else {
            Self::multi_worker(config, storage, parser)
        }
    }

    // Creates a multi-worker builder
    fn multi_worker<T: WorkerConfig>(
        config: &T,
        storage: Storage,
        parser: Parser,
    ) -> Result<WorkersBuilder<Storage, Parser, Request, Response>, Error> {
        let worker_config = config.worker();

        // initialize storage
        let storage = StorageWorkerBuilder::new(config, storage).unwrap_or_else(|e| {
            error!("{}", e);
            std::process::exit(1);
        });

        // initialize workers
        let mut workers = Vec::new();
        for _ in 0..worker_config.threads() {
            let worker = MultiWorkerBuilder::new(config, parser.clone()).unwrap_or_else(|e| {
                error!("{}", e);
                std::process::exit(1);
            });
            workers.push(worker);
        }

        Ok(WorkersBuilder::Multi { storage, workers })
    }

    // Creates a single-worker builder
    fn single_worker<T: WorkerConfig>(
        config: &T,
        storage: Storage,
        parser: Parser,
    ) -> Result<WorkersBuilder<Storage, Parser, Request, Response>, Error> {
        // initialize worker
        let worker = SingleWorkerBuilder::new(config, storage, parser).unwrap_or_else(|e| {
            error!("{}", e);
            std::process::exit(1);
        });

        Ok(WorkersBuilder::Single { worker })
    }

    /// Returns the wakers for all workers. Used when setting-up the queues to
    /// signal to all threads.
    pub(crate) fn wakers(&self) -> Vec<Arc<Waker>> {
        match self {
            Self::Multi { storage, workers } => {
                let mut wakers = vec![storage.waker()];
                for waker in workers.iter().map(|v| v.waker()) {
                    wakers.push(waker);
                }
                wakers
            }
            Self::Single { worker } => {
                vec![worker.waker()]
            }
        }
    }

    /// Returns the wakers for the non-storage workers. Used when setting-up the
    /// queues to send sessions to the workers.
    pub(crate) fn worker_wakers(&self) -> Vec<Arc<Waker>> {
        match self {
            Self::Multi { workers, .. } => workers.iter().map(|v| v.waker()).collect(),
            Self::Single { worker } => {
                vec![worker.waker()]
            }
        }
    }

    /// Converts the builder into the finalized `Workers` type by providing the
    /// necessary queues.
    pub fn build(
        self,
        signal_queues: Vec<Queues<(), Signal>>,
        session_queues: Vec<Queues<(), Session>>,
    ) -> Workers<Storage, Parser, Request, Response> {
        let mut signal_queues = signal_queues;
        let mut session_queues = session_queues;
        match self {
            Self::Multi {
                storage,
                mut workers,
            } => {
                let storage_wakers = vec![storage.waker()];
                let worker_wakers: Vec<Arc<Waker>> = workers.iter().map(|v| v.waker()).collect();
                let (mut response_queues, mut request_queues) =
                    Queues::new(worker_wakers, storage_wakers, QUEUE_CAPACITY);

                // The storage thread precedes the worker threads in the set of
                // wakers, so its signal queue is the first element of
                // `signal_queues`. Its request queue is also the first (and
                // only) element of `request_queues`. We remove these and build
                // the storage so we can loop through the remaining signal
                // queues when launching the worker threads.
                let s = storage.build(signal_queues.remove(0), request_queues.remove(0));

                let mut w = Vec::new();
                for worker_builder in workers.drain(..) {
                    w.push(worker_builder.build(
                        signal_queues.remove(0),
                        session_queues.remove(0),
                        response_queues.remove(0),
                    ));
                }

                Workers::Multi {
                    storage: s,
                    workers: w,
                }
            }
            Self::Single { worker } => Workers::Single {
                worker: worker.build(signal_queues.remove(0), session_queues.remove(0)),
            },
        }
    }
}

/// Represents the finalized `Workers`.
pub enum Workers<Storage, Parser, Request, Response> {
    /// A multi-threaded worker which includes two or more threads to handle
    /// request/response as well as a shared storage thread.
    Multi {
        storage: StorageWorker<Storage, Request, Response>,
        workers: Vec<MultiWorker<Storage, Parser, Request, Response>>,
    },
    /// A single-threaded worker which handles request/response and owns the
    /// storage.
    Single {
        worker: SingleWorker<Storage, Parser, Request, Response>,
    },
}

impl<
        Storage: 'static + Send,
        Parser: 'static + Send,
        Request: 'static + Send,
        Response: 'static + Send,
    > Workers<Storage, Parser, Request, Response>
where
    Parser: Parse<Request>,
    Response: Compose,
    Storage: Execute<Request, Response> + EntryStore,
{
    /// Converts the `Workers` into running threads.
    pub fn spawn(self) -> Vec<JoinHandle<()>> {
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
                threads.push(
                    std::thread::Builder::new()
                        .name(format!("{}_storage", THREAD_PREFIX))
                        .spawn(move || storage.run())
                        .unwrap(),
                );
                for mut worker in workers {
                    let worker_thread = std::thread::Builder::new()
                        .name(format!("{}_worker{}", THREAD_PREFIX, threads.len()))
                        .spawn(move || worker.run())
                        .unwrap();
                    threads.push(worker_thread);
                }
                threads
            }
        }
    }
}
