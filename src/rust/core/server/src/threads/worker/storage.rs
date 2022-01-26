// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use super::*;
use crate::threads::worker::TokenWrapper;
use common::signal::Signal;
use common::time::Instant;
use config::WorkerConfig;
use core::time::Duration;
use entrystore::EntryStore;
use mio::Events;
use mio::Poll;
use mio::Token;
use mio::Waker;
use protocol::{Compose, Execute};
use queues::{QueueError, QueuePair, QueuePairs};
use std::sync::Arc;

// TODO(bmartin): this *should* be plenty safe, the queue should rarely ever be
// full, and a single wakeup should drain at least one message and make room for
// the response. A stat to prove that this is sufficient would be good.
const QUEUE_RETRIES: usize = 3;

const WAKER_TOKEN: usize = usize::MAX;

/// A `Storage` thread is used in a multi-worker configuration. It owns the
/// cache contents and operates on message queues for each worker thread, taking
/// fully parsed requests, processing them, and writing the responses directly
/// into the session write buffers.
pub struct StorageWorker<Storage, Request, Response> {
    poll: Poll,
    nevent: usize,
    timeout: Duration,
    storage: Storage,
    signal_queue: QueuePairs<Signal, Signal>,
    worker_queues: QueuePairs<TokenWrapper<Option<Response>>, TokenWrapper<Request>>,
}

impl<Storage, Request, Response> StorageWorker<Storage, Request, Response>
where
    Request: Send,
    Response: Compose + Send,
    Storage: Execute<Request, Response> + EntryStore + Send,
{
    /// Create a new `Worker` which will get new `Session`s from the MPSC queue
    pub fn new<T: WorkerConfig>(config: &T, storage: Storage) -> Result<Self, std::io::Error> {
        let poll = Poll::new().map_err(|e| {
            error!("{}", e);
            std::io::Error::new(std::io::ErrorKind::Other, "Failed to create epoll instance")
        })?;

        let waker = Arc::new(Waker::new(poll.registry(), Token(WAKER_TOKEN)).unwrap());

        let worker_queues = QueuePairs::new(Some(waker.clone()));
        let signal_queue = QueuePairs::new(Some(waker));

        Ok(Self {
            nevent: config.worker().nevent(),
            timeout: Duration::from_millis(config.worker().timeout() as u64),
            poll,
            storage,
            signal_queue,
            worker_queues,
        })
    }

    /// Add a queue for a worker by providing the worker's `Waker` so that the
    /// worker can be notified of pending responses from the storage thread
    pub fn add_queue(
        &mut self,
        waker: Arc<Waker>,
    ) -> QueuePair<TokenWrapper<Request>, TokenWrapper<Option<Response>>> {
        self.worker_queues.new_pair(65536, Some(waker))
    }

    /// Run the storage thread in a loop, handling incoming messages from the
    /// worker threads
    pub fn run(&mut self) {
        let workers = self.worker_queues.pending().len();

        let mut worker_needs_wake = vec![false; workers];

        let mut events = Events::with_capacity(self.nevent);
        let timeout = Some(self.timeout);

        loop {
            STORAGE_EVENT_LOOP.increment();

            self.storage.expire();

            // get events with timeout
            if self.poll.poll(&mut events, timeout).is_err() {
                error!("Error polling");
            }

            if !events.is_empty() {
                let mut worker_pending = self.worker_queues.pending();
                let total_pending: usize = worker_pending.iter().sum();
                STORAGE_QUEUE_DEPTH.increment(
                    Instant::<Nanoseconds<u64>>::now(),
                    total_pending as _,
                    1,
                );

                trace!("handling events");
                let mut empty = false;

                while !empty {
                    empty = true;
                    for id in 0..workers {
                        if worker_pending[id] > 0 {
                            if let Ok(message) = self.worker_queues.recv_from(id) {
                                trace!("handling request from worker: {}", id);
                                PROCESS_REQ.increment();
                                let token = message.token();
                                let response = self.storage.execute(message.into_inner());
                                let mut message = TokenWrapper::new(response, token);
                                for retry in 0..QUEUE_RETRIES {
                                    if let Err(QueueError::Full(m)) =
                                        self.worker_queues.send_to(id, message)
                                    {
                                        if (retry + 1) == QUEUE_RETRIES {
                                            error!("error sending message to worker");
                                        }
                                        let _ = self.worker_queues.wake(id);
                                        message = m;
                                    } else {
                                        break;
                                    }
                                }
                                worker_needs_wake[id] = true;
                            }
                            empty = false;
                            worker_pending[id] -= 1;
                        }
                    }
                }

                for (id, needs_wake) in worker_needs_wake.iter_mut().enumerate() {
                    if *needs_wake {
                        trace!("waking worker thread: {}", id);
                        let _ = self.worker_queues.wake(id);
                        *needs_wake = false;
                    }
                }

                #[allow(clippy::never_loop)]
                while let Ok(s) = self.signal_queue.recv_from(0) {
                    match s {
                        Signal::Shutdown => {
                            return;
                        }
                    }
                }
            }
        }
    }

    pub fn signal_queue(&mut self) -> QueuePair<Signal, Signal> {
        self.signal_queue.new_pair(128, None)
    }
}
