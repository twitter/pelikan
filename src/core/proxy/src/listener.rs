// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use net::event::Source;
use common::ssl::tls_acceptor;
use crate::*;
use config::proxy::ListenerConfig;
use config::TlsConfig;
use core::time::Duration;
use net::{Poll, Waker};
// use poll::*;
use queues::Queues;
use session_common::*;
use std::sync::Arc;

use rustcommon_metrics::*;

const KB: usize = 1024;

const SESSION_BUFFER_MIN: usize = 16 * KB;
const SESSION_BUFFER_MAX: usize = 1024 * KB;

counter!(LISTENER_EVENT_ERROR);
counter!(LISTENER_EVENT_READ);
counter!(LISTENER_EVENT_WRITE);

pub struct ListenerBuilder {
    addr: SocketAddr,
    nevent: usize,
    listener: net::Listener,
    poll: Poll,
    timeout: Duration,
    sessions: Slab<Session>,
    waker: Arc<Waker>,
}

impl ListenerBuilder {
    pub fn new<T: ListenerConfig + TlsConfig>(config: &T) -> Result<Self> {
        let tls_config = config.tls();
        let config = config.listener();

        let addr = config
            .socket_addr()
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::Other, "bad listen address"))?;

        let tcp_listener = TcpListener::bind(addr)?;

        let nevent = config.nevent();
        let timeout = Duration::from_millis(config.timeout() as u64);

        let listener = if let Some(tls_acceptor) = tls_acceptor(tls_config)? {
            net::Listener::from((tcp_listener, tls_acceptor))
        } else {
            net::Listener::from(tcp_listener)
        };

        let mut poll = Poll::new()?;
        listener.register(poll.registry(), LISTENER_TOKEN, net::Interest::READABLE)?;

        let waker = Arc::new(Waker::new(poll.registry(), WAKER_TOKEN).unwrap());

        Ok(Self {
            addr,
            nevent,
            listener,
            poll,
            sessions: Slab::new(),
            timeout,
            waker,
        })
    }

    pub fn waker(&self) -> Arc<Waker> {
        self.waker.clone()
    }

    pub fn build(self, connection_queues: Queues<Session, ()>) -> Listener {
        Listener {
            addr: self.addr,
            connection_queues,
            nevent: self.nevent,
            poll: self.poll,
            timeout: self.timeout,
            listener: self.listener,
            sessions: self.sessions,
            waker: self.waker
        }
    }
}

pub struct Listener {
    addr: SocketAddr,
    connection_queues: Queues<Session, ()>,
    nevent: usize,
    listener: net::Listener,
    sessions: Slab<Session>,
    poll: Poll,
    timeout: Duration,
    waker: Arc<Waker>,
}

impl Listener {
    /// Handle an event on an existing session
    fn handle_session_event(&mut self, event: &Event) {
        let token = event.token();

        // handle error events first
        if event.is_error() {
            LISTENER_EVENT_ERROR.increment();
            let _ = self.sessions.remove(token.0);
        }

        // read events are handled last
        if event.is_readable() {
            LISTENER_EVENT_READ.increment();
            let _ = self.do_read(token);
        }

        if let Some(session) = self.sessions.get_mut(token.0) {
            if session.do_handshake().is_ok() {
                trace!("handshake complete for session: {:?}", session);
                let session = self.sessions.remove(token.0);
                if self
                    .connection_queues
                    .try_send_any(session)
                    .is_err()
                {
                    error!("error sending session to worker");
                    TCP_ACCEPT_EX.increment();
                }
            } else {
                trace!("handshake incomplete for session: {:?}", session);
            }
        }
    }

    /// Handle a read event for the `Session` with the `Token`.
    pub fn do_read(&mut self, token: Token) {
        if let Some(session) = self.sessions.get_mut(token.0) {
            // read from session to buffer
            match session.fill() {
                Ok(0) => {
                    trace!("hangup for session: {:?}", session);
                    let _ = self.sessions.remove(token.0);
                }
                Ok(bytes) => {
                    trace!("read {} bytes for session: {:?}", bytes, session);
                }
                Err(e) => {
                    match e.kind() {
                        ErrorKind::WouldBlock => {
                            // spurious read, ignore
                        }
                        ErrorKind::Interrupted => {
                            // this should be retried immediately
                            trace!("interrupted");
                            self.do_read(token)
                        }
                        _ => {
                            // some read error
                            trace!("closing session due to read error: {:?} {:?}", session, e);
                            let _ = session.flush();
                            let _ = self.sessions.remove(token.0);
                        }
                    }
                }
            }
        } else {
            warn!("attempted to read from non-existent session: {}", token.0);
        }
    }

    pub fn do_accept(&mut self) {
        if let Ok(session) = self.listener.accept().map(|v| Session::from(v)) {
            if !session.is_handshaking() {
                self.connection_queues.try_send_any(session);
            } else {
                let s = self.sessions.vacant_entry();
                session.register(self.poll.registry(), Token(s.key()), session.interest());
                s.insert(session);
            }
        }

        self.listener.reregister(self.poll.registry(), LISTENER_TOKEN, net::Interest::READABLE);
        let _ = self.connection_queues.wake();
    }

    pub fn run(mut self) {
        info!("running listener on: {}", self.addr);

        let mut events = Events::with_capacity(self.nevent);
        loop {
            let _ = self.poll.poll(&mut events, Some(self.timeout));
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
