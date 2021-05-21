// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::threads::*;
use common::signal::Signal;
use entrystore::EntryStore;
use mio::Waker;
use protocol::{Compose, Execute, Parse};
use queues::QueuePair;
use session::Session;
use std::sync::Arc;
use std::thread::JoinHandle;

const THREAD_PREFIX: &str = "pelikan";

/// Wraps specialization of launching single or multi-threaded worker(s)
pub enum WorkerBuilder<Storage, Request, Response>
where
    Request: Parse,
    Response: Compose,
    Storage: Execute<Request, Response> + EntryStore,
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
    Storage: Execute<Request, Response> + EntryStore + Send,
{
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

    pub fn signal_queues(&mut self) -> Vec<QueuePair<Signal, ()>> {
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
