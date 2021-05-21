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

pub struct QueuePairs<T, U> {
    queue_pairs: Vec<QueuePair<T, U>>,
    waker: Option<Arc<Waker>>,
    next: usize,
}

impl<T, U> QueuePairs<T, U> {
    pub fn new(waker: Option<Arc<Waker>>) -> Self {
        Self {
            queue_pairs: Vec::new(),
            waker,
            next: 0,
        }
    }

    pub fn pending(&self) -> Box<[usize]> {
        self.queue_pairs
            .iter()
            .map(|queue_pair| queue_pair.pending())
            .collect::<Vec<usize>>()
            .into_boxed_slice()
    }

    pub fn recv_from(&mut self, id: usize) -> Result<U, QueueError<T>> {
        if let Some(queue_pair) = self.queue_pairs.get_mut(id) {
            queue_pair.try_recv().map_err(|_| QueueError::Empty)
        } else {
            Err(QueueError::NoQueue)
        }
    }

    pub fn send_to(&mut self, id: usize, msg: T) -> Result<(), QueueError<T>> {
        if let Some(queue) = self.queue_pairs.get_mut(id) {
            match queue.try_send(msg) {
                Ok(()) => Ok(()),
                Err(SendError::Full(msg)) => Err(QueueError::Full(msg)),
            }
        } else {
            Err(QueueError::NoQueue)
        }
    }

    pub fn send_rr(&mut self, mut msg: T) -> Result<(), QueueError<T>> {
        let queue_pairs = self.queue_pairs.len();
        if queue_pairs == 0 {
            return Err(QueueError::NoQueue);
        }
        for _ in 0..queue_pairs {
            if self.next >= queue_pairs {
                self.next = 0;
            }
            match self.queue_pairs[self.next].try_send(msg) {
                Ok(()) => {
                    let _ = self.queue_pairs[self.next].wake();
                    self.next = self.next.wrapping_add(1);
                    return Ok(());
                }
                Err(SendError::Full(m)) => {
                    self.next = self.next.wrapping_add(1);
                    msg = m;
                }
            }
        }
        Err(QueueError::Full(msg))
    }

    pub fn wake(&self, id: usize) -> Result<(), QueueError<T>> {
        if let Some(queue_pair) = self.queue_pairs.get(id) {
            queue_pair.wake().map_err(QueueError::WakeFailed)
        } else {
            Err(QueueError::NoQueue)
        }
    }

    pub fn wake_all(&self) -> Result<(), QueueError<T>> {
        for queue_pair in &self.queue_pairs {
            queue_pair.wake().map_err(QueueError::WakeFailed)?;
        }
        Ok(())
    }

    pub fn new_pair(&mut self, capacity: usize, waker: Option<Arc<Waker>>) -> QueuePair<U, T> {
        let (theirs, ours) = queue_pair_with_capacity(capacity, waker, self.waker.clone());
        self.queue_pairs.push(ours);
        theirs
    }

    pub fn add_pair(&mut self, queue_pair: QueuePair<T, U>) {
        self.queue_pairs.push(queue_pair)
    }
}

impl<T: Clone, U> QueuePairs<T, U> {
    pub fn broadcast(&mut self, msg: T) -> Result<(), QueueError<T>> {
        if self.queue_pairs.is_empty() {
            return Err(QueueError::NoQueue);
        }
        let mut success = true;
        for queue_pair in &mut self.queue_pairs {
            if queue_pair.try_send(msg.clone()).is_err() {
                success = false;
            }
        }
        if success {
            Ok(())
        } else {
            Err(QueueError::Full(msg))
        }
    }
}

pub enum QueueError<T> {
    Empty,
    NoQueue,
    Full(T),
    WakeFailed(std::io::Error),
}
