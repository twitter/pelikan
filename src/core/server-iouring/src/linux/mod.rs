// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use std::io::Result;
use std::os::unix::io::{AsRawFd,};
use std::sync::mpsc::*;
use std::sync::Arc;

pub const TIMEOUT_TOKEN: u64 = u64::MAX - 1;
pub const LISTENER_TOKEN: u64 = u64::MAX;

mod listener;
mod session;
mod waker;
mod worker;

pub use listener::{Listener, ListenerBuilder};
pub use session::{Session, State};
use waker::Waker;
pub use worker::{Worker, WorkerBuilder};

pub struct Queue<T, U> {
    tx: Sender<T>,
    rx: Receiver<U>,
    waker: Arc<Waker>,
}

impl<T, U> Queue<T, U>
where
    T: Send,
    U: Send,
{
    pub fn send(&self, item: T) -> std::result::Result<(), T> {
        self.tx.send(item).map_err(|e| e.0)
    }

    pub fn try_recv(&self) -> std::result::Result<U, ()> {
        self.rx.try_recv().map_err(|e| ())
    }

    pub fn wake(&self) -> Result<()> {
        self.waker.wake()
    }
}

pub fn queues<T, U>(a_waker: Arc<Waker>, b_waker: Arc<Waker>) -> (Queue<T, U>, Queue<U, T>) {
    let (t_tx, t_rx) = channel();
    let (u_tx, u_rx) = channel();

    let a = Queue {
        tx: t_tx,
        rx: u_rx,
        waker: b_waker,
    };

    let b = Queue {
        tx: u_tx,
        rx: t_rx,
        waker: a_waker,
    };

    (a, b)
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
