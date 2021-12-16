// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::*;
use rtrb::*;
use std::sync::Arc;

/// A bi-directional channel for communication between two threads.
pub struct QueuePair<T, U> {
    send: Producer<T>,
    recv: Consumer<U>,
    waker: Option<Arc<Waker>>,
}

/// Creats a new queue pair that can hold up to capacity items in each
/// direction. The optional `Waker`s allow the sender to inform the receiver
/// that a receive operation should return at least one message.
pub fn queue_pair_with_capacity<A, B>(
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
    pub fn with_capacity(
        capacity: usize,
        waker_a: Option<Arc<Waker>>,
        waker_b: Option<Arc<Waker>>,
    ) -> (QueuePair<T, U>, QueuePair<U, T>) {
        queue_pair_with_capacity(capacity, waker_a, waker_b)
    }

    /// Attempt to send a message over the queue pair.
    pub fn try_send(&mut self, msg: T) -> Result<(), SendError<T>> {
        match self.send.push(msg) {
            Ok(()) => Ok(()),
            Err(PushError::Full(msg)) => Err(SendError::Full(msg)),
        }
    }

    /// Try to receive a message from the queue pair.
    pub fn try_recv(&mut self) -> Result<U, RecvError> {
        self.recv.pop().map_err(|_| RecvError::Empty)
    }

    /// Notify the receiver that a receive operation should return a message.
    pub fn wake(&self) -> Result<(), std::io::Error> {
        if let Some(ref waker) = self.waker {
            waker.wake()
        } else {
            Ok(())
        }
    }

    /// Return the number of pending messages on the receive side of the queue
    /// pair.
    pub fn pending(&self) -> usize {
        self.recv.slots()
    }
}
