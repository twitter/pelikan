// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

mod error;

pub use error::*;

pub use mio::Waker;
use rtrb::*;
use std::sync::Arc;

pub struct QueuePair<T, U> {
    send: Producer<T>,
    recv: Consumer<U>,
    waker: Option<Arc<Waker>>,
}

fn queue_pair_with_capacity<A, B>(
    capacity: usize,
    waker_a: Option<Arc<Waker>>,
    waker_b: Option<Arc<Waker>>,
) -> (QueuePair<A, B>, QueuePair<B, A>) {
    let (to_a, from_b) = rtrb::RingBuffer::new(capacity).split();
    let (to_b, from_a) = rtrb::RingBuffer::new(capacity).split();

    let queue_a = QueuePair::<A, B> {
        send: to_b,
        recv: from_b,
        waker: waker_b,
    };

    let queue_b = QueuePair::<B, A> {
        send: to_a,
        recv: from_a,
        waker: waker_a,
    };

    (queue_a, queue_b)
}

impl<T, U> QueuePair<T, U> {
    pub fn try_send(&mut self, msg: T) -> Result<(), SendError<T>> {
        match self.send.push(msg) {
            Ok(()) => Ok(()),
            Err(PushError::Full(msg)) => Err(SendError::Full(msg)),
        }
    }

    pub fn try_recv(&mut self) -> Result<U, RecvError> {
        self.recv.pop().map_err(|_| RecvError::Empty)
    }

    // Notify the receiver that messages are on the queue
    pub fn wake(&self) -> Result<(), std::io::Error> {
        if let Some(ref waker) = self.waker {
            waker.wake()
        } else {
            Ok(())
        }
    }

    pub fn pending(&self) -> usize {
        self.recv.slots()
    }
}

pub struct MultiQueuePair<T, U> {
    queues: Vec<QueuePair<T, U>>,
    waker: Option<Arc<Waker>>,
    next: usize,
}

impl<T, U> MultiQueuePair<T, U> {
    pub fn new(waker: Option<Arc<Waker>>) -> Self {
        Self {
            queues: Vec::new(),
            waker,
            next: 0,
        }
    }

    pub fn pending(&self) -> Box<[usize]> {
        self.queues
            .iter()
            .map(|queue| queue.pending())
            .collect::<Vec<usize>>()
            .into_boxed_slice()
    }

    pub fn recv_from(&mut self, id: usize) -> Result<U, MultiQueueError<T>> {
        if let Some(queue) = self.queues.get_mut(id) {
            queue.try_recv().map_err(|_| MultiQueueError::Empty)
        } else {
            Err(MultiQueueError::NoQueue)
        }
    }

    pub fn send_to(&mut self, id: usize, msg: T) -> Result<(), MultiQueueError<T>> {
        if let Some(queue) = self.queues.get_mut(id) {
            match queue.try_send(msg) {
                Ok(()) => Ok(()),
                Err(SendError::Full(msg)) => Err(MultiQueueError::Full(msg)),
            }
        } else {
            Err(MultiQueueError::NoQueue)
        }
    }

    pub fn send_rr(&mut self, mut msg: T) -> Result<(), MultiQueueError<T>> {
        let queues = self.queues.len();
        if queues == 0 {
            return Err(MultiQueueError::NoQueue);
        }
        for _ in 0..queues {
            if self.next >= self.queues.len() {
                self.next %= self.queues.len();
            }
            match self.queues[self.next].try_send(msg) {
                Ok(()) => {
                    let _ = self.queues[self.next].wake();
                    self.next = self.next.wrapping_add(1);
                    return Ok(());
                }
                Err(SendError::Full(m)) => {
                    self.next = self.next.wrapping_add(1);
                    msg = m;
                }
            }
        }
        Err(MultiQueueError::Full(msg))
    }

    pub fn wake(&self, id: usize) -> Result<(), MultiQueueError<T>> {
        if let Some(queue) = self.queues.get(id) {
            queue.wake().map_err(MultiQueueError::WakeFailed)
        } else {
            Err(MultiQueueError::NoQueue)
        }
    }

    pub fn new_queue_pair(
        &mut self,
        capacity: usize,
        waker: Option<Arc<Waker>>,
    ) -> QueuePair<U, T> {
        let (theirs, ours) = queue_pair_with_capacity(capacity, waker, self.waker.clone());
        self.queues.push(ours);
        theirs
    }

    pub fn register_queue_pair(&mut self, queue: QueuePair<T, U>) {
        self.queues.push(queue)
    }
}

pub enum MultiQueueError<T> {
    Empty,
    NoQueue,
    Full(T),
    WakeFailed(std::io::Error),
}
