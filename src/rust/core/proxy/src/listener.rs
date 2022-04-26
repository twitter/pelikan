// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::*;
use mio::Waker;
use poll::*;
use queues::Queues;
use session::Session;
use std::sync::Arc;

pub struct ListenerBuilder {
    addr: SocketAddr,
    poll: Poll,
}

impl ListenerBuilder {
    pub fn new(addr: SocketAddr) -> Result<Self> {
        let mut poll = Poll::new()?;
        poll.bind(addr)?;
        Ok(Self { addr, poll })
    }

    pub fn waker(&self) -> Arc<Waker> {
        self.poll.waker()
    }

    pub fn build(self, connection_queues: Queues<Session, ()>) -> Listener {
        Listener {
            addr: self.addr,
            poll: self.poll,
            connection_queues,
        }
    }
}

pub struct Listener {
    addr: SocketAddr,
    poll: Poll,
    connection_queues: Queues<Session, ()>,
}

impl Listener {
    pub fn run(mut self) {
        info!("running listener on: {}", self.addr);

        let mut events = Events::with_capacity(1024);
        loop {
            let _ = self
                .poll
                .poll(&mut events, core::time::Duration::from_millis(100));
            for event in &events {
                match event.token() {
                    LISTENER_TOKEN => {
                        while let Ok((stream, _addr)) = self.poll.accept() {
                            let session =
                                Session::plain_with_capacity(stream, 16 * 1024, 1024 * 1024);
                            // self.poll.deregister(session);
                            // let connection = ClientConnection::new(addr, stream);
                            if self.connection_queues.try_send_any(session).is_err() {
                                warn!("rejecting connection, client connection queue is too full");
                            } else {
                                trace!("sending new connection to worker threads");
                            }
                            let _ = self.connection_queues.wake();
                        }
                    }
                    WAKER_TOKEN => {}
                    token => {
                        warn!("listener: unexpected event for token: {}", token.0);
                    }
                }
            }
        }
    }
}
