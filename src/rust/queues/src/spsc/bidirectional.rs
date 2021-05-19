// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Shared structs and helper functions.

use mio::Waker;
use std::sync::Arc;
use rtrb::*;

pub use rtrb::PopError;
pub use rtrb::PushError;

pub struct Bidirectional<T, U> {
    send: Producer<T>,
    recv: Consumer<U>,
    waker: Arc<Waker>,
}

pub fn with_capacity<A, B>(capacity: usize, waker_a: Arc<Waker>, waker_b: Arc<Waker>) -> (Bidirectional<A, B>, Bidirectional<B, A>) {
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
    pub fn try_send(&mut self, msg: T) -> Result<(), PushError<T>> {
        self.send.push(msg)
    }

    pub fn try_recv(&mut self) -> Result<U, PopError> {
        self.recv.pop()
    }

    // Notify the receiver that messages are on the queue
    pub fn wake(&self) -> Result<(), std::io::Error> {
        self.waker.wake()
    }

    pub fn pending(&self) -> usize {
        self.recv.slots()
    }
}