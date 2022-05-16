// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::*;
use config::proxy::ListenerConfig;
use config::TlsConfig;
use core::time::Duration;
use mio::Waker;
use poll::*;
use queues::Queues;
use session::Session;
use std::sync::Arc;

const KB: usize = 1024;

const SESSION_BUFFER_MIN: usize = 16 * KB;
const SESSION_BUFFER_MAX: usize = 1024 * KB;

static_metrics! {
    static LISTENER_EVENT_ERROR: Counter;
    static LISTENER_EVENT_READ: Counter;
    static LISTENER_EVENT_WRITE: Counter;
}


pub struct ListenerBuilder {
    addr: SocketAddr,
    nevent: usize,
    poll: Poll,
    timeout: Duration,
}

impl ListenerBuilder {
    pub fn new<T: ListenerConfig + TlsConfig>(config: &T) -> Result<Self> {
        let tls_config = config.tls();
        let config = config.listener();

        let addr = config
            .socket_addr()
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::Other, "bad listen address"))?;
        let nevent = config.nevent();
        let timeout = Duration::from_millis(config.timeout() as u64);

        let mut poll = Poll::new()?;
        poll.bind(addr, tls_config)?;

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
    /// Handle an event on an existing session
    fn handle_session_event(&mut self, event: &Event) {
        let token = event.token();

        // handle error events first
        if event.is_error() {
            LISTENER_EVENT_ERROR.increment();
            self.handle_error(token);
        }

        // handle write events before read events to reduce write
        // buffer growth if there is also a readable event
        if event.is_writable() {
            LISTENER_EVENT_WRITE.increment();
            self.do_write(token);
        }

        // read events are handled last
        if event.is_readable() {
            LISTENER_EVENT_READ.increment();
            let _ = self.do_read(token);
        }

        if let Ok(session) = self.poll.get_mut_session(token) {
            if session.session.do_handshake().is_ok() {
                trace!("handshake complete for session: {:?}", session.session);
                if let Ok(session) = self.poll.remove_session(token) {
                    if self.connection_queues.try_send_any(session.session).is_err() {
                        error!("error sending session to worker");
                        TCP_ACCEPT_EX.increment();
                    }
                } else {
                    error!("error removing session from poller");
                    TCP_ACCEPT_EX.increment();
                }
            } else {
                trace!("handshake incomplete for session: {:?}", session.session);
            }
        }
    }

    pub fn do_accept(&mut self) {
        if let Ok(token) = self.poll.accept() {
            match self.poll.get_mut_session(token).map(|v| v.session.is_handshaking()) {
                Ok(false) => {
                    if let Ok(session) = self.poll.remove_session(token) {
                        if self.connection_queues.try_send_any(session.session).is_err() {
                            warn!("rejecting connection, client connection queue is too full");
                        } else {
                            trace!("sending new connection to worker threads");
                        }
                    }
                }
                Ok(true) => {},
                Err(e) => {
                    warn!("error checking if new session is handshaking: {}", e);
                }
            }
        }
        self.poll.reregister(LISTENER_TOKEN);
        let _ = self.connection_queues.wake();
    }

    pub fn run(mut self) {
        info!("running listener on: {}", self.addr);

        let mut events = Events::with_capacity(self.nevent);
        loop {
            let _ = self.poll.poll(&mut events, self.timeout);
            for event in &events {
                match event.token() {
                    LISTENER_TOKEN => {
                       self.do_accept();
                    }
                    WAKER_TOKEN => {}
                    _ => {
                        self.handle_session_event(event);
                    }
                }
            }
        }
    }
}

impl EventLoop for Listener {
    fn handle_data(&mut self, _token: Token) -> Result<()> {
        Ok(())
    }

    fn poll(&mut self) -> &mut Poll {
        &mut self.poll
    }
}
