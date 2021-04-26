// Copyright 2020 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use ahash::RandomState;
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
    message_receiver: Receiver<Message>,
    message_sender: Sender<Message>,
    worker_sender: Vec<Producer<StorageMessage>>,
    worker_receiver: Vec<Consumer<StorageMessage>>,
    worker_waker: Vec<Waker>,
}

pub struct CacheHasher {
    inner: ahash::RandomState,
}

impl Default for CacheHasher {
    fn default() -> Self {
        let inner = RandomState::with_seeds(
            0xbb8c484891ec6c86,
            0x0522a25ae9c769f9,
            0xeed2797b9571bc75,
            0x4feb29c1fbbd59d0,
        );
        Self { inner }
    }
}

impl BuildHasher for CacheHasher {
    type Hasher = ahash::AHasher;

    fn build_hasher(&self) -> Self::Hasher {
        self.inner.build_hasher()
    }
}

pub struct StorageQueue {
    sender: Producer<StorageMessage>,
    receiver: Consumer<StorageMessage>,
}

impl StorageQueue {
    pub fn try_recv(&mut self) -> Result<StorageMessage, PopError> {
        self.receiver.pop()
    }

    pub fn try_send(&mut self, msg: StorageMessage) -> Result<(), PushError<StorageMessage>> {
        self.sender.push(msg)
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
            .hash_extra_capacity(config.segcache().hash_extra_capacity())
            .heap_size(config.segcache().heap_size())
            .segment_size(config.segcache().segment_size())
            .eviction(eviction)
            .hasher(CacheHasher::default())
            .build();

        Ok(Self {
            config,
            data,
            message_receiver,
            message_sender,
            worker_sender: Vec::new(),
            worker_receiver: Vec::new(),
            worker_waker: Vec::new(),
        })
    }

    pub fn add_queue(&mut self, waker: Waker) -> StorageQueue {
        let (to_storage, from_worker) = rtrb::RingBuffer::new(65536).split();
        let (to_worker, from_storage) = rtrb::RingBuffer::new(65536).split();

        self.worker_sender.push(to_worker);
        self.worker_receiver.push(from_worker);
        self.worker_waker.push(waker);
        StorageQueue {
            sender: to_storage,
            receiver: from_storage,
        }
    }

    /// Run the `Worker` in a loop, handling new session events
    pub fn run(&mut self) {
        let mut last = CoarseInstant::now();
        let mut worker_needs_wake = vec![false; self.worker_waker.len()];
        loop {
            increment_counter!(&Stat::StorageEventLoop);

            self.data.expire();

            loop {
                let now = CoarseInstant::now();
                if now != last {
                    last = now;
                    break;
                }

                let mut empty = false;

                while !empty {
                    empty = true;
                    for (id, queue) in self.worker_receiver.iter_mut().enumerate() {
                        if let Ok(mut message) = queue.pop() {
                            if let Some(request) = message.request.take() {
                                increment_counter!(&Stat::ProcessReq);
                                match request {
                                    Request::Get(r) => {
                                        process_get(r, &mut message.buffer, &mut self.data);
                                    }
                                    Request::Gets(r) => {
                                        process_gets(r, &mut message.buffer, &mut self.data);
                                    }
                                    Request::Set(r) => {
                                        process_set(
                                            &self.config,
                                            r,
                                            &mut message.buffer,
                                            &mut self.data,
                                        );
                                    }
                                    Request::Cas(r) => {
                                        process_cas(
                                            &self.config,
                                            r,
                                            &mut message.buffer,
                                            &mut self.data,
                                        );
                                    }
                                    Request::Add(r) => {
                                        process_add(
                                            &self.config,
                                            r,
                                            &mut message.buffer,
                                            &mut self.data,
                                        );
                                    }
                                    Request::Replace(r) => {
                                        process_replace(
                                            &self.config,
                                            r,
                                            &mut message.buffer,
                                            &mut self.data,
                                        );
                                    }
                                    Request::Delete(r) => {
                                        process_delete(r, &mut message.buffer, &mut self.data);
                                    }
                                }
                            }
                            self.worker_sender[id].push(message).unwrap();
                            worker_needs_wake[id] = true;
                            empty = false;
                        }
                    }
                }

                for (id, needs_wake) in worker_needs_wake.iter_mut().enumerate() {
                    if *needs_wake {
                        let _ = self.worker_waker[id].wake();
                        *needs_wake = false;
                    }
                }

                std::thread::sleep(std::time::Duration::from_micros(100));
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
