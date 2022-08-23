// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::*;
use std::thread::JoinHandle;

mod multi_worker;
mod single_worker;
mod storage_worker;

use multi_worker::*;
use single_worker::*;
use storage_worker::*;

fn map_result(result: Result<usize>) -> Result<()> {
    match result {
        Ok(0) => Err(Error::new(ErrorKind::Other, "client hangup")),
        Ok(_) => Ok(()),
        Err(e) => map_err(e),
    }
}

fn map_err(e: std::io::Error) -> Result<()> {
    match e.kind() {
        ErrorKind::WouldBlock => Ok(()),
        _ => Err(e),
    }
}

pub enum Workers<Parser, Request, Response, Storage> {
    Single {
        worker: SingleWorker<Parser, Request, Response, Storage>,
    },
    Multi {
        workers: Vec<MultiWorker<Parser, Request, Response>>,
        storage: StorageWorker<Request, Response, Storage>,
    },
}

impl<Parser, Request, Response, Storage> Workers<Parser, Request, Response, Storage>
where
    Parser: 'static + Parse<Request> + Clone + Send,
    Request: 'static + Send,
    Response: 'static + Compose + Send,
    Storage: 'static + EntryStore + Execute<Request, Response> + Send,
{
    pub fn spawn(self) -> Vec<JoinHandle<()>> {
        match self {
            Self::Single { mut worker } => {
                vec![std::thread::Builder::new()
                    .name(format!("{}_worker", THREAD_PREFIX))
                    .spawn(move || worker.run())
                    .unwrap()]
            }
            Self::Multi {
                mut workers,
                mut storage,
            } => {
                let mut join_handles = vec![std::thread::Builder::new()
                    .name(format!("{}_storage", THREAD_PREFIX))
                    .spawn(move || storage.run())
                    .unwrap()];

                for (id, mut worker) in workers.drain(..).enumerate() {
                    join_handles.push(
                        std::thread::Builder::new()
                            .name(format!("{}_worker_{}", THREAD_PREFIX, id))
                            .spawn(move || worker.run())
                            .unwrap(),
                    )
                }

                join_handles
            }
        }
    }
}

pub enum WorkersBuilder<Parser, Request, Response, Storage> {
    Single {
        worker: SingleWorkerBuilder<Parser, Request, Response, Storage>,
    },
    Multi {
        workers: Vec<MultiWorkerBuilder<Parser, Request, Response>>,
        storage: StorageWorkerBuilder<Request, Response, Storage>,
    },
}

impl<Parser, Request, Response, Storage> WorkersBuilder<Parser, Request, Response, Storage>
where
    Parser: Parse<Request> + Clone,
    Response: Compose,
    Storage: Execute<Request, Response> + EntryStore,
{
    pub fn new<T: WorkerConfig>(config: &T, parser: Parser, storage: Storage) -> Result<Self> {
        let threads = config.worker().threads();

        if threads > 1 {
            let mut workers = vec![];
            for _ in 0..threads {
                workers.push(MultiWorkerBuilder::new(config, parser.clone())?)
            }

            Ok(Self::Multi {
                workers,
                storage: StorageWorkerBuilder::new(config, storage)?,
            })
        } else {
            Ok(Self::Single {
                worker: SingleWorkerBuilder::new(config, parser, storage)?,
            })
        }
    }

    pub fn worker_wakers(&self) -> Vec<Arc<Box<dyn waker::Waker>>> {
        match self {
            Self::Single { worker } => {
                vec![worker.waker()]
            }
            Self::Multi {
                workers,
                storage: _,
            } => workers.iter().map(|w| w.waker()).collect(),
        }
    }

    pub fn wakers(&self) -> Vec<Arc<Box<dyn waker::Waker>>> {
        match self {
            Self::Single { worker } => {
                vec![worker.waker()]
            }
            Self::Multi { workers, storage } => {
                let mut wakers = vec![storage.waker()];
                for worker in workers {
                    wakers.push(worker.waker());
                }
                wakers
            }
        }
    }

    pub fn build(
        self,
        session_queues: Vec<Queues<Session, Session>>,
        signal_queues: Vec<Queues<(), Signal>>,
    ) -> Workers<Parser, Request, Response, Storage> {
        let mut signal_queues = signal_queues;
        let mut session_queues = session_queues;
        match self {
            Self::Multi {
                storage,
                mut workers,
            } => {
                let storage_wakers = vec![storage.waker()];
                let worker_wakers: Vec<Arc<Box<dyn waker::Waker>>> =
                    workers.iter().map(|v| v.waker()).collect();
                let (mut worker_data_queues, mut storage_data_queues) =
                    Queues::new(worker_wakers, storage_wakers, QUEUE_CAPACITY);

                // The storage thread precedes the worker threads in the set of
                // wakers, so its signal queue is the first element of
                // `signal_queues`. Its request queue is also the first (and
                // only) element of `request_queues`. We remove these and build
                // the storage so we can loop through the remaining signal
                // queues when launching the worker threads.
                let s = storage.build(storage_data_queues.remove(0), signal_queues.remove(0));

                let mut w = Vec::new();
                for worker_builder in workers.drain(..) {
                    w.push(worker_builder.build(
                        worker_data_queues.remove(0),
                        session_queues.remove(0),
                        signal_queues.remove(0),
                    ));
                }

                Workers::Multi {
                    storage: s,
                    workers: w,
                }
            }
            Self::Single { worker } => Workers::Single {
                worker: worker.build(session_queues.remove(0), signal_queues.remove(0)),
            },
        }
    }
}
