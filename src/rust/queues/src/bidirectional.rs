// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Shared structs and helper functions.

use mio::Waker;
use rtrb::*;
use std::sync::Arc;

use rtrb::PushError;

pub enum RecvError {
    Empty,
}

pub enum SendError<T> {
    Full(T)
}

pub struct Bidirectional<T, U> {
    send: Producer<T>,
    recv: Consumer<U>,
    waker: Option<Arc<Waker>>,
}

pub fn with_capacity<A, B>(
    capacity: usize,
    waker_a: Option<Arc<Waker>>,
    waker_b: Option<Arc<Waker>>,
) -> (Bidirectional<A, B>, Bidirectional<B, A>) {
    let (to_a, from_b) = rtrb::RingBuffer::new(capacity).split();
    let (to_b, from_a) = rtrb::RingBuffer::new(capacity).split();

    let queue_a = Bidirectional::<A, B> {
        send: to_b,
        recv: from_b,
        waker: waker_b,
    };

    let queue_b = Bidirectional::<B, A> {
        send: to_a,
        recv: from_a,
        waker: waker_a,
    };

    (queue_a, queue_b)
}

impl<T, U> Bidirectional<T, U> {
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
