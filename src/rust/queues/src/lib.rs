// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Queue type for inter-process communication (IPC).

pub use mio::Waker;

use crossbeam_channel::*;
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
    receiver: Receiver<TrackedItem<U>>,
    id: usize,
    rng: ChaCha20Rng,
    distr: Uniform<usize>,
}

pub struct WakingSender<T> {
    inner: Sender<T>,
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
        let result = self.inner.try_send(item).map_err(|e| e.into_inner());
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

impl<T, U> Queues<T, U> {
    pub fn new(
        senders: Vec<Arc<Waker>>,
        receivers: Vec<Arc<Waker>>,
    ) -> (Vec<Queues<T, U>>, Vec<Queues<U, T>>) {
        let mut senders = senders;
        let mut receivers = receivers;

        // T messages are sent to the receivers
        let mut send_tx = Vec::<WakingSender<TrackedItem<T>>>::with_capacity(receivers.len());
        let mut recv_tx = Vec::<Receiver<TrackedItem<T>>>::with_capacity(receivers.len());

        for waker in receivers.drain(..) {
            let (s, r) = bounded(1024);
            let s = WakingSender {
                inner: s,
                waker,
                needs_wake: false,
            };
            send_tx.push(s);
            recv_tx.push(r);
        }

        // U messages sent are sent to the senders
        let mut send_rx = Vec::<WakingSender<TrackedItem<U>>>::with_capacity(senders.len());
        let mut recv_rx = Vec::<Receiver<TrackedItem<U>>>::with_capacity(senders.len());

        for waker in senders.drain(..) {
            let (s, r) = bounded(1024);
            let s = WakingSender {
                inner: s,
                waker,
                needs_wake: false,
            };
            send_rx.push(s);
            recv_rx.push(r);
        }

        let mut s = Vec::new();
        let mut r = Vec::new();

        for (id, receiver) in recv_rx.drain(..).enumerate() {
            s.push(Queues {
                senders: send_tx.clone(),
                receiver,
                rng: ChaCha20Rng::from_entropy(),
                distr: Uniform::new(0, send_tx.len()),
                id,
            })
        }

        for (id, receiver) in recv_tx.drain(..).enumerate() {
            r.push(Queues {
                senders: send_rx.clone(),
                receiver,
                rng: ChaCha20Rng::from_entropy(),
                distr: Uniform::new(0, send_rx.len()),
                id,
            })
        }

        (s, r)
    }

    /// Try to receive a single item from the queue
    pub fn try_recv(&self) -> Result<TrackedItem<U>, TryRecvError> {
        self.receiver.try_recv()
    }

    /// Try to receive all pending items from the queue
    pub fn try_recv_all(&self, buf: &mut Vec<TrackedItem<U>>) {
        let pending = self.receiver.len();
        for _ in 0..pending {
            if let Ok(item) = self.receiver.try_recv() {
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
    use mio::*;
    use std::sync::Arc;

    const WAKER_TOKEN: Token = Token(usize::MAX);

    #[test]
    fn basic() {
        let poll = Poll::new().expect("failed to create event loop");
        let waker =
            Arc::new(Waker::new(poll.registry(), WAKER_TOKEN).expect("failed to create waker"));

        let (mut a, mut b) = Queues::<usize, String>::new(vec![waker.clone()], vec![waker]);
        let mut a = a.remove(0);
        let mut b = b.remove(0);

        // queues are empty
        assert!(a.try_recv().is_err());
        assert!(b.try_recv().is_err());

        // send a usize from A -> B using a targeted send
        a.try_send_to(0, 1).expect("failed to send");
        assert!(a.try_recv().is_err());
        assert_eq!(
            b.try_recv().map(|v| (v.sender(), v.into_inner())),
            Ok((0, 1))
        );

        // queues are empty
        assert!(a.try_recv().is_err());
        assert!(b.try_recv().is_err());

        // send a usize from A -> B using a non-targeted (any) send
        a.try_send_any(2).expect("failed to send");
        assert!(a.try_recv().is_err());
        assert_eq!(
            b.try_recv().map(|v| (v.sender(), v.into_inner())),
            Ok((0, 2))
        );

        // queues are empty
        assert!(a.try_recv().is_err());
        assert!(b.try_recv().is_err());

        // send a usize from A -> B using a broadcast send
        a.try_send_all(3).expect("failed to send");
        assert!(a.try_recv().is_err());
        assert_eq!(
            b.try_recv().map(|v| (v.sender(), v.into_inner())),
            Ok((0, 3))
        );

        // queues are empty
        assert!(a.try_recv().is_err());
        assert!(b.try_recv().is_err());

        // send a String from B -> A using a targeted send
        b.try_send_to(0, "apple".to_string())
            .expect("failed to send");
        assert!(b.try_recv().is_err());
        assert_eq!(
            a.try_recv().map(|v| (v.sender(), v.into_inner())),
            Ok((0, "apple".to_string()))
        );

        // queues are empty
        assert!(a.try_recv().is_err());
        assert!(b.try_recv().is_err());

        // send a usize from A -> B using a non-targeted (any) send
        b.try_send_any("banana".to_string())
            .expect("failed to send");
        assert!(b.try_recv().is_err());
        assert_eq!(
            a.try_recv().map(|v| (v.sender(), v.into_inner())),
            Ok((0, "banana".to_string()))
        );

        // queues are empty
        assert!(a.try_recv().is_err());
        assert!(b.try_recv().is_err());

        // send a usize from A -> B using a broadcast send
        b.try_send_all("orange".to_string())
            .expect("failed to send");
        assert!(b.try_recv().is_err());
        assert_eq!(
            a.try_recv().map(|v| (v.sender(), v.into_inner())),
            Ok((0, "orange".to_string()))
        );
    }
}
