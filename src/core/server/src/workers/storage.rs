// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::*;

counter!(
    STORAGE_EVENT_LOOP,
    "the number of times the event loop has run"
);
heatmap!(
    STORAGE_QUEUE_DEPTH,
    1_000_000,
    "the distribution of the depth of the storage queue on each loop"
);

pub struct StorageWorkerBuilder<Request, Response, Storage> {
    nevent: usize,
    poll: Poll,
    storage: Storage,
    timeout: Duration,
    waker: Arc<Box<dyn waker::Waker>>,
    _request: PhantomData<Request>,
    _response: PhantomData<Response>,
}

impl<Request, Response, Storage> StorageWorkerBuilder<Request, Response, Storage> {
    pub fn new<T: WorkerConfig>(config: &T, storage: Storage) -> Result<Self> {
        let config = config.worker();

        let poll = Poll::new()?;

        let waker =
            Arc::new(Box::new(Waker::new(poll.registry(), WAKER_TOKEN).unwrap())
                as Box<dyn waker::Waker>);

        let nevent = config.nevent();
        let timeout = Duration::from_millis(config.timeout() as u64);

        Ok(Self {
            nevent,
            poll,
            storage,
            timeout,
            waker,
            _request: PhantomData,
            _response: PhantomData,
        })
    }

    pub fn waker(&self) -> Arc<Box<dyn waker::Waker>> {
        self.waker.clone()
    }

    pub fn build(
        self,
        data_queue: Queues<(Request, Response, Token), (Request, Token)>,
        signal_queue: Queues<(), Signal>,
    ) -> StorageWorker<Request, Response, Storage, Token> {
        StorageWorker {
            data_queue,
            nevent: self.nevent,
            poll: self.poll,
            signal_queue,
            storage: self.storage,
            timeout: self.timeout,
            waker: self.waker,
            _request: PhantomData,
            _response: PhantomData,
        }
    }
}

pub struct StorageWorker<Request, Response, Storage, Token> {
    data_queue: Queues<(Request, Response, Token), (Request, Token)>,
    nevent: usize,
    poll: Poll,
    signal_queue: Queues<(), Signal>,
    storage: Storage,
    timeout: Duration,
    #[allow(dead_code)]
    waker: Arc<Box<dyn waker::Waker>>,
    _request: PhantomData<Request>,
    _response: PhantomData<Response>,
}

impl<Request, Response, Storage, Token> StorageWorker<Request, Response, Storage, Token>
where
    Storage: Execute<Request, Response> + EntryStore,
    Request: Klog + Klog<Response = Response>,
    Response: Compose,
{
    /// Run the `StorageWorker` in a loop, handling new session events.
    pub fn run(&mut self) {
        let mut events = Events::with_capacity(self.nevent);
        let mut messages = Vec::with_capacity(1024);

        loop {
            STORAGE_EVENT_LOOP.increment();

            self.storage.expire();

            // get events with timeout
            if self.poll.poll(&mut events, Some(self.timeout)).is_err() {
                error!("Error polling");
            }

            let timestamp = Instant::now();

            if !events.is_empty() {
                trace!("handling events");

                self.data_queue.try_recv_all(&mut messages);

                STORAGE_QUEUE_DEPTH.increment(timestamp, messages.len() as _, 1);

                for message in messages.drain(..) {
                    let sender = message.sender();
                    let (request, token) = message.into_inner();
                    trace!("handling request from worker: {}", sender);
                    let response = self.storage.execute(&request);
                    PROCESS_REQ.increment();
                    let mut message = (request, response, token);
                    for retry in 0..QUEUE_RETRIES {
                        if let Err(m) = self.data_queue.try_send_to(sender, message) {
                            if (retry + 1) == QUEUE_RETRIES {
                                error!("error sending message to worker");
                            }
                            // wake workers immediately
                            let _ = self.data_queue.wake();
                            message = m;
                        } else {
                            break;
                        }
                    }
                }

                let _ = self.data_queue.wake();

                // check if we received any signals from the admin thread
                while let Some(s) = self.signal_queue.try_recv().map(|v| v.into_inner()) {
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
