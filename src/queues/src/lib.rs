// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Queue type for inter-process communication (IPC).

pub use net::Waker;

use crossbeam_queue::*;
use rand::distributions::Uniform;
use rand::Rng as RandRng;
use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;
use std::sync::Arc;

/// A struct for sending and receiving items by using very simple routing. This
/// allows for us to send messages to a specific receiver, to any receiver, or
/// all receivers. Automatically wraps items with the identifier of the sender
/// so that a response can be sent back to the corresponding receiver.
pub struct Queues<T, U> {
    senders: Vec<WakingSender<TrackedItem<T>>>,
    receiver: Arc<ArrayQueue<TrackedItem<U>>>,
    id: usize,
    rng: ChaCha20Rng,
    distr: Uniform<usize>,
}

struct WakingSender<T> {
    inner: Arc<ArrayQueue<T>>,
    waker: Arc<Waker>,
    needs_wake: bool,
}

impl<T> Clone for WakingSender<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            waker: self.waker.clone(),
            needs_wake: false,
        }
    }
}

impl<T> std::fmt::Debug for WakingSender<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        write!(f, "{:?}", self.inner)
    }
}

impl<T> WakingSender<T> {
    pub fn try_send(&mut self, item: T) -> Result<(), T> {
        let result = self.inner.push(item);
        if result.is_ok() {
            self.needs_wake = true;
        }
        result
    }

    pub fn wake(&mut self) -> Result<(), std::io::Error> {
        if self.needs_wake {
            let result = self.waker.wake();
            if result.is_ok() {
                self.needs_wake = false;
            }
            result
        } else {
            Ok(())
        }
    }
}

/// The `Queues` type allows sending items of one type, and receiving items of
/// another type. This allows for bi-directional communication between threads
/// where a transformation of the messages from one type to another may be
/// performed.
///
/// For instance, this can allow for sending requests from one set of threads to
/// be processed by another set of threads. The responses can then be sent back
/// to the original thread which dispatched the request.
///
/// This type allows directed one-to-one communication, undirected one-to-any
/// communication, or broadcast one-to-all communication.
impl<T, U> Queues<T, U> {
    /// Construct the queues for communicating between both sides. Side `a`
    /// sends items of type `T` to side `b`. Side `b` sends items of type `U` to
    /// side `a`.
    ///
    /// To construct this type, you must pass the `mio::Waker`s registered for
    /// each `mio::Poll` instance for side `a` and side `b`. This is required so
    /// that the queues can be used within the event loops and wakeups for the
    /// receivers can be issued by the senders.
    ///
    /// Since bounded queues are used internally, the capacity is the maximum
    /// number of pending items each receiver can have. For example, if side A
    /// has 4 queues, and a capacity of 1024*4 => 4096 items of type U may be
    /// pending.
    ///
    /// NOTE: the return vectors maintain the ordering of the wakers that were
    /// provided. Care must be taken to ensure that the corresponding queues are
    /// given to the event loop with the corresponding waker.
    pub fn new<A: AsRef<[Arc<Waker>]>, B: AsRef<[Arc<Waker>]>>(
        a_wakers: A,
        b_wakers: B,
        capacity: usize,
    ) -> (Vec<Queues<T, U>>, Vec<Queues<U, T>>) {
        let mut a_wakers = a_wakers.as_ref().to_vec();
        let mut b_wakers = b_wakers.as_ref().to_vec();

        // T messages are sent to side b, so we have a `WakingSender` for each
        // side b queue.

        // these will be used in side a to transmit
        let mut a_tx = Vec::<WakingSender<TrackedItem<T>>>::with_capacity(b_wakers.len());

        // these will be used in side b to receive
        let mut b_rx = Vec::<Arc<ArrayQueue<TrackedItem<T>>>>::with_capacity(b_wakers.len());

        for waker in b_wakers.drain(..) {
            let q = Arc::new(ArrayQueue::new(capacity));
            let s = WakingSender {
                inner: q.clone(),
                waker,
                needs_wake: false,
            };
            a_tx.push(s);
            b_rx.push(q);
        }

        // T messages are sent to side b, so we have a `WakingSender` for each
        // side a queue.

        // these will be used in side b to transmit
        let mut b_tx = Vec::<WakingSender<TrackedItem<U>>>::with_capacity(a_wakers.len());

        // these will be used in side a to receive
        let mut a_rx = Vec::<Arc<ArrayQueue<TrackedItem<U>>>>::with_capacity(a_wakers.len());

        for waker in a_wakers.drain(..) {
            let q = Arc::new(ArrayQueue::new(capacity));
            let s = WakingSender {
                inner: q.clone(),
                waker,
                needs_wake: false,
            };
            b_tx.push(s);
            a_rx.push(q);
        }

        let mut a = Vec::new();
        let mut b = Vec::new();

        for (id, receiver) in a_rx.drain(..).enumerate() {
            a.push(Queues {
                senders: a_tx.clone(),
                receiver,
                rng: ChaCha20Rng::from_entropy(),
                distr: Uniform::new(0, a_tx.len()),
                id,
            })
        }

        for (id, receiver) in b_rx.drain(..).enumerate() {
            b.push(Queues {
                senders: b_tx.clone(),
                receiver,
                rng: ChaCha20Rng::from_entropy(),
                distr: Uniform::new(0, b_tx.len()),
                id,
            })
        }

        (a, b)
    }

    /// Try to receive a single item from the queue. Returns a `TrackedItem<T>`
    /// which allows the receiver to know which sender sent the item.
    pub fn try_recv(&self) -> Option<TrackedItem<U>> {
        self.receiver.pop()
    }

    /// Try to receive all pending items from the queue.
    pub fn try_recv_all(&self, buf: &mut Vec<TrackedItem<U>>) {
        let pending = self.receiver.len();
        for _ in 0..pending {
            if let Some(item) = self.receiver.pop() {
                buf.push(item);
            }
        }
    }

    /// Try to send a single item to the receiver specified by the `id`. Allows
    /// targeted 1:1 communication.
    ///
    /// This can be used when we need to send a response back to the sender of
    /// a `TrackedItem`. For example, if we receive a request, do some
    /// processing, and need to send a response back to the sending thread.
    pub fn try_send_to(&mut self, id: usize, item: T) -> Result<(), T> {
        self.senders[id]
            .try_send(TrackedItem {
                sender: self.id,
                inner: item,
            })
            .map_err(|e| e.into_inner())
    }

    /// Try to send a single item to any receiver. Uses a uniform random
    /// distribution to pick a receiver. Allows balanced 1:N communication.
    ///
    /// This can be used when it doesn't matter which receiver gets the item,
    /// but it is desirable to have items spread evenly across receivers. For
    /// example, this can be used to send accepted TCP streams to worker threads
    /// in a manner that is roughly balanced.
    pub fn try_send_any(&mut self, item: T) -> Result<(), T> {
        let id = self.rng.sample(self.distr);
        self.senders[id]
            .try_send(TrackedItem {
                sender: self.id,
                inner: item,
            })
            .map_err(|e| e.into_inner())
    }

    /// Wake any remote receivers which have been sent items since the last time
    /// this was called.
    pub fn wake(&mut self) -> Result<(), std::io::Error> {
        let mut result = Ok(());
        for sender in self.senders.iter_mut() {
            if let Err(e) = sender.wake() {
                result = Err(e);
            }
        }
        result
    }
}

impl<T: Clone, U> Queues<T, U> {
    /// Allows broadcast communication of the item to all receivers on the other
    /// side.
    pub fn try_send_all(&mut self, item: T) -> Result<(), T> {
        let mut result = Ok(());
        for sender in self.senders.iter_mut() {
            if sender
                .try_send(TrackedItem {
                    sender: self.id,
                    inner: item.clone(),
                })
                .is_err()
            {
                result = Err(item.clone());
            }
        }
        result
    }
}

pub struct TrackedItem<T> {
    sender: usize,
    inner: T,
}

impl<T> TrackedItem<T> {
    pub fn sender(&self) -> usize {
        self.sender
    }
    pub fn into_inner(self) -> T {
        self.inner
    }
}

#[cfg(test)]
mod tests {
    use crate::Queues;
    use ::net::*;
    use std::sync::Arc;

    const WAKER_TOKEN: Token = Token(usize::MAX);

    #[test]
    fn basic() {
        let poll = Poll::new().expect("failed to create event loop");
        let waker =
            Arc::new(Waker::new(poll.registry(), WAKER_TOKEN).expect("failed to create waker"));

        let (mut a, mut b) = Queues::<usize, String>::new(vec![waker.clone()], vec![waker], 1024);
        let mut a = a.remove(0);
        let mut b = b.remove(0);

        // queues are empty
        assert!(a.try_recv().is_none());
        assert!(b.try_recv().is_none());

        // send a usize from A -> B using a targeted send
        a.try_send_to(0, 1).expect("failed to send");
        assert!(a.try_recv().is_none());
        assert_eq!(
            b.try_recv().map(|v| (v.sender(), v.into_inner())),
            Some((0, 1))
        );

        // queues are empty
        assert!(a.try_recv().is_none());
        assert!(b.try_recv().is_none());

        // send a usize from A -> B using a non-targeted (any) send
        a.try_send_any(2).expect("failed to send");
        assert!(a.try_recv().is_none());
        assert_eq!(
            b.try_recv().map(|v| (v.sender(), v.into_inner())),
            Some((0, 2))
        );

        // queues are empty
        assert!(a.try_recv().is_none());
        assert!(b.try_recv().is_none());

        // send a usize from A -> B using a broadcast send
        a.try_send_all(3).expect("failed to send");
        assert!(a.try_recv().is_none());
        assert_eq!(
            b.try_recv().map(|v| (v.sender(), v.into_inner())),
            Some((0, 3))
        );

        // queues are empty
        assert!(a.try_recv().is_none());
        assert!(b.try_recv().is_none());

        // send a String from B -> A using a targeted send
        b.try_send_to(0, "apple".to_string())
            .expect("failed to send");
        assert!(b.try_recv().is_none());
        assert_eq!(
            a.try_recv().map(|v| (v.sender(), v.into_inner())),
            Some((0, "apple".to_string()))
        );

        // queues are empty
        assert!(a.try_recv().is_none());
        assert!(b.try_recv().is_none());

        // send a usize from A -> B using a non-targeted (any) send
        b.try_send_any("banana".to_string())
            .expect("failed to send");
        assert!(b.try_recv().is_none());
        assert_eq!(
            a.try_recv().map(|v| (v.sender(), v.into_inner())),
            Some((0, "banana".to_string()))
        );

        // queues are empty
        assert!(a.try_recv().is_none());
        assert!(b.try_recv().is_none());

        // send a usize from A -> B using a broadcast send
        b.try_send_all("orange".to_string())
            .expect("failed to send");
        assert!(b.try_recv().is_none());
        assert_eq!(
            a.try_recv().map(|v| (v.sender(), v.into_inner())),
            Some((0, "orange".to_string()))
        );
    }
}
