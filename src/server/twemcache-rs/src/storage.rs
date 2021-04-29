// Copyright 2020 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use bytes::BytesMut;
use config::segcache::Eviction;
use crossbeam_channel::Receiver;
use crossbeam_channel::Sender;
use metrics::Stat;
use rtrb::*;
use rustcommon_time::CoarseInstant;
use segcache::{Policy, SegCache};

use core::hash::BuildHasher;
use std::sync::Arc;

use crate::common::Message;
use crate::protocol::data::*;
use crate::*;

pub struct StorageMessage {
    pub request: Option<Request>,
    pub buffer: BytesMut,
    pub token: Token,
}

pub struct Storage<S>
where
    S: BuildHasher,
{
    config: Arc<Config>,
    data: SegCache<S>,
    poll: Poll,
    message_receiver: Receiver<Message>,
    message_sender: Sender<Message>,
    waker: Arc<Waker>,
    worker_sender: Vec<Producer<StorageMessage>>,
    worker_receiver: Vec<Consumer<StorageMessage>>,
    worker_waker: Vec<Waker>,
}

pub struct StorageQueue {
    sender: Producer<StorageMessage>,
    receiver: Consumer<StorageMessage>,
    waker: Arc<Waker>,
}

impl StorageQueue {
    // Try to receive a message back from the storage queue, returned messages
    // will contain the session write buffer with the response appended.
    pub fn try_recv(&mut self) -> Result<StorageMessage, PopError> {
        self.receiver.pop()
    }

    // Try to send a message to the storage queue. Messages should contain the
    // parsed request and the session write buffer.
    pub fn try_send(&mut self, msg: StorageMessage) -> Result<(), PushError<StorageMessage>> {
        self.sender.push(msg)
    }

    // Notify the storage thread that it should wake and handle messages.
    pub fn wake(&self) -> Result<(), std::io::Error> {
        self.waker.wake()
    }
}

impl Storage<CacheHasher> {
    /// Create a new `Worker` which will get new `Session`s from the MPSC queue
    pub fn new(config: Arc<Config>) -> Result<Self, std::io::Error> {
        let (message_sender, message_receiver) = crossbeam_channel::bounded(128);

        let eviction = match config.segcache().eviction() {
            Eviction::None => Policy::None,
            Eviction::Random => Policy::Random,
            Eviction::Fifo => Policy::Fifo,
            Eviction::Cte => Policy::Cte,
            Eviction::Util => Policy::Util,
            Eviction::Merge => Policy::Merge {
                max: config.segcache().merge_max(),
                merge: config.segcache().merge_target(),
                compact: config.segcache().compact_target(),
            },
        };

        let data = SegCache::builder()
            .power(config.segcache().hash_power())
            .overflow_factor(config.segcache().overflow_factor())
            .heap_size(config.segcache().heap_size())
            .segment_size(config.segcache().segment_size())
            .eviction(eviction)
            .hasher(CacheHasher::default())
            .build();

        let poll = Poll::new().map_err(|e| {
            error!("{}", e);
            std::io::Error::new(std::io::ErrorKind::Other, "Failed to create epoll instance")
        })?;

        let waker = Arc::new(Waker::new(poll.registry(), Token(usize::MAX)).unwrap());

        Ok(Self {
            config,
            data,
            poll,
            message_receiver,
            message_sender,
            waker,
            worker_sender: Vec::new(),
            worker_receiver: Vec::new(),
            worker_waker: Vec::new(),
        })
    }

    /// Add a queue for a worker by providing the worker's `Waker` so that the
    /// worker can be notified of pending responses from the storage thread
    pub fn add_queue(&mut self, waker: Waker) -> StorageQueue {
        let (to_storage, from_worker) = rtrb::RingBuffer::new(65536).split();
        let (to_worker, from_storage) = rtrb::RingBuffer::new(65536).split();

        self.worker_sender.push(to_worker);
        self.worker_receiver.push(from_worker);
        self.worker_waker.push(waker);
        StorageQueue {
            sender: to_storage,
            receiver: from_storage,
            waker: self.waker.clone(),
        }
    }

    /// Run the storage thread in a loop, handling incoming messages from the
    /// worker threads
    pub fn run(&mut self) {
        // holds the number of workers registered
        let workers = self.worker_waker.len();

        // holds state about whether a given worker needs a deferred wake, this
        // is used to coalesce wakeups and reduce syscall load
        let mut worker_needs_wake = vec![false; workers];

        // holds state about how many messages were pending for each worker when
        // a wakeup happened
        let mut worker_pending = vec![0; workers];

        let mut events = Events::with_capacity(self.config.worker().nevent());
        let timeout = Some(std::time::Duration::from_millis(
            self.config.worker().timeout() as u64,
        ));

        loop {
            increment_counter!(&Stat::StorageEventLoop);

            self.data.expire();

            // get events with timeout
            if self.poll.poll(&mut events, timeout).is_err() {
                error!("Error polling");
            }

            if !events.is_empty() {
                // store the number of messages currently in each queue when
                // wakeup occurred
                for (id, queue) in self.worker_receiver.iter_mut().enumerate() {
                    worker_pending[id] = queue.slots();
                }

                let mut empty = false;

                while !empty {
                    empty = true;
                    for id in 0..workers {
                        if worker_pending[id] > 0 {
                            if let Ok(mut message) = self.worker_receiver[id].pop() {
                                if let Some(request) = message.request.take() {
                                    increment_counter!(&Stat::ProcessReq);
                                    request.process(&self.config, &mut message.buffer, &mut self.data);
                                    self.worker_sender[id].push(message).unwrap();
                                    worker_needs_wake[id] = true;
                                }
                            }
                            empty = false;
                            worker_pending[id] -= 1;
                        }
                    }
                }

                for (id, needs_wake) in worker_needs_wake.iter_mut().enumerate() {
                    if *needs_wake {
                        let _ = self.worker_waker[id].wake();
                        *needs_wake = false;
                    }
                }
            }

            // poll queue to receive new messages
            #[allow(clippy::never_loop)]
            while let Ok(message) = self.message_receiver.try_recv() {
                match message {
                    Message::Shutdown => {
                        return;
                    }
                }
            }
        }
    }

    pub fn message_sender(&self) -> Sender<Message> {
        self.message_sender.clone()
    }
}
