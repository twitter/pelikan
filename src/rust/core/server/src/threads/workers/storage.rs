// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use super::*;
use crate::poll::Poll;
use common::signal::Signal;
use config::WorkerConfig;
use core::time::Duration;
use entrystore::EntryStore;
use mio::Events;
use mio::Waker;
use protocol::Execute;
use std::marker::PhantomData;
use std::sync::Arc;

// TODO(bmartin): this *should* be plenty safe, the queue should rarely ever be
// full, and a single wakeup should drain at least one message and make room for
// the response. A stat to prove that this is sufficient would be good.
const QUEUE_RETRIES: usize = 3;

/// A builder type for a storage worker which owns the storage and executes
/// requests from a queue and returns responses back to the worker threads.
pub struct StorageWorkerBuilder<Storage, Request, Response> {
    poll: Poll,
    nevent: usize,
    timeout: Duration,
    storage: Storage,
    _request: PhantomData<Request>,
    _response: PhantomData<Response>,
}

impl<Storage, Request, Response> StorageWorkerBuilder<Storage, Request, Response> {
    /// Create a new `StorageWorkerBuilder` from the config and storage.
    pub fn new<T: WorkerConfig>(config: &T, storage: Storage) -> Result<Self, std::io::Error> {
        let poll = Poll::new().map_err(|e| {
            error!("{}", e);
            std::io::Error::new(std::io::ErrorKind::Other, "Failed to create epoll instance")
        })?;

        Ok(Self {
            nevent: config.worker().nevent(),
            timeout: Duration::from_millis(config.worker().timeout() as u64),
            poll,
            storage,
            _request: PhantomData,
            _response: PhantomData,
        })
    }

    /// Returns the waker for the storage worker.
    pub(crate) fn waker(&self) -> Arc<Waker> {
        self.poll.waker()
    }

    /// Finalize the builder and return a `StorageWorker` by providing the
    /// queues that are necessary for communication with other threads.
    pub fn build(
        self,
        signal_queue: Queues<(), Signal>,
        storage_queue: Queues<TokenWrapper<Option<Response>>, TokenWrapper<Request>>,
    ) -> StorageWorker<Storage, Request, Response> {
        StorageWorker {
            poll: self.poll,
            nevent: self.nevent,
            timeout: self.timeout,
            signal_queue,
            storage: self.storage,
            storage_queue,
        }
    }
}

/// A finalized `StorageWorker` which is ready to be run.
pub struct StorageWorker<Storage, Request, Response> {
    poll: Poll,
    nevent: usize,
    timeout: Duration,
    signal_queue: Queues<(), Signal>,
    storage: Storage,
    storage_queue: Queues<TokenWrapper<Option<Response>>, TokenWrapper<Request>>,
}

impl<Storage, Request, Response> StorageWorker<Storage, Request, Response>
where
    Storage: Execute<Request, Response> + EntryStore,
{
    /// Run the `StorageWorker` in a loop, handling new session events.
    pub fn run(&mut self) {
        let mut events = Events::with_capacity(self.nevent);
        let mut requests = Vec::with_capacity(1024);

        loop {
            STORAGE_EVENT_LOOP.increment();

            self.storage.expire();

            // get events with timeout
            if self.poll.poll(&mut events, self.timeout).is_err() {
                error!("Error polling");
            }

            if !events.is_empty() {
                trace!("handling events");

                self.storage_queue.try_recv_all(&mut requests);

                STORAGE_QUEUE_DEPTH.increment(
                    common::time::Instant::<common::time::Nanoseconds<u64>>::now(),
                    requests.len() as _,
                    1,
                );

                for request in requests.drain(..) {
                    let sender = request.sender();
                    let request = request.into_inner();
                    trace!("handling request from worker: {}", sender);
                    PROCESS_REQ.increment();
                    let token = request.token();
                    let response = self.storage.execute(request.into_inner());
                    let mut message = TokenWrapper::new(response, token);
                    for retry in 0..QUEUE_RETRIES {
                        if let Err(m) = self.storage_queue.try_send_to(sender, message) {
                            if (retry + 1) == QUEUE_RETRIES {
                                error!("error sending message to worker");
                            }
                            let _ = self.storage_queue.wake();
                            message = m;
                        } else {
                            break;
                        }
                    }
                }

                let _ = self.storage_queue.wake();

                #[allow(clippy::never_loop)]
                // check if we received any signals from the admin thread
                while let Ok(s) = self.signal_queue.try_recv().map(|v| v.into_inner()) {
                    match s {
                        Signal::FlushAll => {
                            warn!("received flush_all");
                            self.storage.clear();
                        }
                        Signal::Shutdown => {
                            // if we received a shutdown, we can return and stop
                            // processing events

                            // TODO(bmartin): graceful shutdown would occur here
                            // when we add persistence

                            return;
                        }
                    }
                }
            }
        }
    }
}
