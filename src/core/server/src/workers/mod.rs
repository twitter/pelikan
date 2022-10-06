// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::*;
use std::thread::JoinHandle;

mod multi;
mod single;

use multi::*;
use single::*;

heatmap!(
    WORKER_EVENT_DEPTH,
    100_000,
    "distribution of the number of events received per iteration of the event loop"
);
counter!(WORKER_EVENT_ERROR, "the number of error events received");
counter!(
    WORKER_EVENT_LOOP,
    "the number of times the event loop has run"
);
counter!(
    WORKER_EVENT_MAX_REACHED,
    "the number of times the maximum number of events was returned"
);
counter!(WORKER_EVENT_READ, "the number of read events received");
counter!(WORKER_EVENT_TOTAL, "the total number of events received");
counter!(WORKER_EVENT_WRITE, "the number of write events received");

fn map_result(result: Result<usize>) -> Result<()> {
    match result {
        Ok(0) => Err(Error::new(ErrorKind::Other, "client hangup")),
        Ok(_) => Ok(()),
        Err(e) => map_err(e),
    }
}

pub enum Workers<Parser, Request, Response, Storage> {
    Single {
        worker: SingleWorker<Parser, Request, Response, Storage>,
    },
    Multi {
        workers: Vec<MultiWorker<Parser, Request, Response, Storage>>,
    },
}

impl<Parser, Request, Response, Storage> Workers<Parser, Request, Response, Storage>
where
    Parser: 'static + Parse<Request> + Clone + Send,
    Request: 'static + Klog + Klog<Response = Response> + Send,
    Response: 'static + Compose + IntoBuffers + Send,
    Storage: 'static + EntryStore + Execute<Request, Response> + Send,
{
    pub fn spawn(self) -> Vec<JoinHandle<()>> {
        match self {
            Self::Single { mut worker } => {
                vec![std::thread::Builder::new()
                    .name(format!("{}_work", THREAD_PREFIX))
                    .spawn(move || worker.run())
                    .unwrap()]
            }
            Self::Multi { mut workers } => {
                let mut join_handles = vec![];

                for (id, mut worker) in workers.drain(..).enumerate() {
                    join_handles.push(
                        std::thread::Builder::new()
                            .name(format!("{}_work_{}", THREAD_PREFIX, id))
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
        workers: Vec<MultiWorkerBuilder<Parser, Request, Response, Storage>>,
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
            let storage = Arc::new(Mutex::new(storage));

            let mut workers = vec![];
            for _ in 0..threads {
                workers.push(MultiWorkerBuilder::new(
                    config,
                    parser.clone(),
                    storage.clone(),
                )?)
            }

            Ok(Self::Multi { workers })
        } else {
            Ok(Self::Single {
                worker: SingleWorkerBuilder::new(config, parser, storage)?,
            })
        }
    }

    pub fn worker_wakers(&self) -> Vec<Arc<Waker>> {
        match self {
            Self::Single { worker } => {
                vec![worker.waker()]
            }
            Self::Multi { workers } => workers.iter().map(|w| w.waker()).collect(),
        }
    }

    pub fn wakers(&self) -> Vec<Arc<Waker>> {
        match self {
            Self::Single { worker } => {
                vec![worker.waker()]
            }
            Self::Multi { workers } => {
                let mut wakers = vec![];
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
                // storage,
                mut workers,
            } => {
                let mut w = Vec::new();
                for worker_builder in workers.drain(..) {
                    w.push(worker_builder.build(session_queues.remove(0), signal_queues.remove(0)));
                }

                Workers::Multi { workers: w }
            }
            Self::Single { worker } => Workers::Single {
                worker: worker.build(session_queues.remove(0), signal_queues.remove(0)),
            },
        }
    }
}
