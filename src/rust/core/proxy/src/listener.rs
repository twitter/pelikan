// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::*;
use config::proxy::ListenerConfig;
use core::time::Duration;
use mio::Waker;
use poll::*;
use queues::Queues;
use session::Session;
use std::sync::Arc;

const KB: usize = 1024;

const SESSION_BUFFER_MIN: usize = 16 * KB;
const SESSION_BUFFER_MAX: usize = 1024 * KB;

pub struct ListenerBuilder {
    addr: SocketAddr,
    nevent: usize,
    poll: Poll,
    timeout: Duration,
}

impl ListenerBuilder {
    pub fn new<T: ListenerConfig>(config: &T) -> Result<Self> {
        let config = config.listener();

        let addr = config
            .socket_addr()
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::Other, "bad listen address"))?;
        let nevent = config.nevent();
        let timeout = Duration::from_millis(config.timeout() as u64);

        let mut poll = Poll::new()?;
        poll.bind(addr)?;

        Ok(Self {
            addr,
            nevent,
            poll,
            timeout,
        })
    }

    pub fn waker(&self) -> Arc<Waker> {
        self.poll.waker()
    }

    pub fn build(self, connection_queues: Queues<Session, ()>) -> Listener {
        Listener {
            addr: self.addr,
            connection_queues,
            nevent: self.nevent,
            poll: self.poll,
            timeout: self.timeout,
        }
    }
}

pub struct Listener {
    addr: SocketAddr,
    connection_queues: Queues<Session, ()>,
    nevent: usize,
    poll: Poll,
    timeout: Duration,
}

impl Listener {
    pub fn run(mut self) {
        info!("running listener on: {}", self.addr);

        let mut events = Events::with_capacity(self.nevent);
        loop {
            let _ = self.poll.poll(&mut events, self.timeout);
            for event in &events {
                match event.token() {
                    LISTENER_TOKEN => {
                        // TODO(bmartin): this assumes plaintext connections
                        while let Ok((stream, _addr)) = self.poll.accept() {
                            let session = Session::plain_with_capacity(
                                stream,
                                SESSION_BUFFER_MIN,
                                SESSION_BUFFER_MAX,
                            );
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
