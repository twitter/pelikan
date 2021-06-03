// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::*;
use queue_pair::queue_pair_with_capacity;
use std::sync::Arc;

/// A collection of queue pairs which can be used to allow a thread to exchange
/// messages with several threads.
pub struct QueuePairs<T, U> {
    queue_pairs: Vec<QueuePair<T, U>>,
    waker: Option<Arc<Waker>>,
    next: usize,
}

impl<T, U> QueuePairs<T, U> {
    /// Create a new collection of queue pairs. The provided `Waker` will be
    /// used to signal that there is one or more queue pairs with pending
    /// messages.
    pub fn new(waker: Option<Arc<Waker>>) -> Self {
        Self {
            queue_pairs: Vec::new(),
            waker,
            next: 0,
        }
    }

    /// Returns the number of pending messages on the receive side for each
    /// queue.
    pub fn pending(&self) -> Box<[usize]> {
        self.queue_pairs
            .iter()
            .map(|queue_pair| queue_pair.pending())
            .collect::<Vec<usize>>()
            .into_boxed_slice()
    }

    /// Try to read from the queue pair with the specified queue id.
    pub fn recv_from(&mut self, id: usize) -> Result<U, QueueError<T>> {
        if let Some(queue_pair) = self.queue_pairs.get_mut(id) {
            queue_pair.try_recv().map_err(|_| QueueError::Empty)
        } else {
            Err(QueueError::NoQueue)
        }
    }

    /// Try to send a message using the queue pair with the specified queue id.
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

    /// Try to send a message in a round-robin fashion to the first queue with
    /// capacity.
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

    /// Wake the receiver for the queue with the specified queue id.
    pub fn wake(&self, id: usize) -> Result<(), QueueError<T>> {
        if let Some(queue_pair) = self.queue_pairs.get(id) {
            queue_pair.wake().map_err(QueueError::WakeFailed)
        } else {
            Err(QueueError::NoQueue)
        }
    }

    /// Wake all queue pair receivers.
    pub fn wake_all(&self) -> Result<(), QueueError<T>> {
        for queue_pair in &self.queue_pairs {
            queue_pair.wake().map_err(QueueError::WakeFailed)?;
        }
        Ok(())
    }

    /// Create a new queue pair with the specified capacity and remote `Waker`.
    pub fn new_pair(&mut self, capacity: usize, waker: Option<Arc<Waker>>) -> QueuePair<U, T> {
        let (theirs, ours) = queue_pair_with_capacity(capacity, waker, self.waker.clone());
        self.queue_pairs.push(ours);
        theirs
    }

    /// Add an already initialized queue pair to this collection.
    pub fn add_pair(&mut self, queue_pair: QueuePair<T, U>) {
        self.queue_pairs.push(queue_pair)
    }
}

impl<T: Clone, U> QueuePairs<T, U> {
    /// Send a message to all queue pairs.
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
