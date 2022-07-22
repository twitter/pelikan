// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! This module provides common functionality for threads which are based on an
//! event loop.

use std::fmt::Debug;
use session_common::Session;
use net::TcpListener;
use net::Listener;
use crate::TCP_ACCEPT_EX;
use common::ssl::*;
use net::event::{Events, Source};
use net::{Interest, Token, Waker};
// use session_legacy::{Session, TcpStream};
use slab::Slab;
use std::convert::TryFrom;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

pub const LISTENER_TOKEN: Token = Token(usize::MAX - 1);
pub const WAKER_TOKEN: Token = Token(usize::MAX);

const KB: usize = 1024;

const SESSION_BUFFER_MIN: usize = 16 * KB;
const SESSION_BUFFER_MAX: usize = 1024 * KB;

pub struct Poll<S> {
    listener: Option<Listener>,
    poll: net::Poll,
    sessions: Slab<TrackedSession<S>>,
    waker: Arc<Waker>,
}

pub struct TrackedSession<S> {
    pub session: S,
    pub sender: Option<usize>,
    pub token: Option<Token>,
}

impl<S> Debug for TrackedSession<S> where S: Debug {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        write!(f, "{:?}", self.session)
    }
}

impl<S> Poll<S>
where
    S: net::event::Source + Debug
{
    /// Create a new `Poll` instance.
    pub fn new() -> Result<Self, std::io::Error> {
        let poll = net::Poll::new().map_err(|e| {
            error!("{}", e);
            std::io::Error::new(std::io::ErrorKind::Other, "failed to create poll instance")
        })?;

        let waker = Arc::new(Waker::new(poll.registry(), WAKER_TOKEN).unwrap());

        let sessions = Slab::<TrackedSession>::new();

        Ok(Self {
            listener: None,
            poll,
            sessions,
            waker,
        })
    }

    /// Bind and begin listening on the provided address.
    pub fn bind(
        &mut self,
        addr: SocketAddr,
        tls_config: &dyn TlsConfig,
    ) -> Result<(), std::io::Error> {
        let mut listener = TcpListener::bind(addr).map_err(|e| {
            error!("{}", e);
            std::io::Error::new(std::io::ErrorKind::Other, "failed to start tcp listener")
        })?;

        let listener = if let Some(acceptor) = common::ssl::tls_acceptor(tls_config)? {
            Listener::from((listener, acceptor))
        } else {
            Listener::from(listener)
        };

        // register listener to event loop
        self.poll
            .registry()
            .register(&mut listener, LISTENER_TOKEN, Interest::READABLE)
            .map_err(|e| {
                error!("{}", e);
                std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "failed to register listener with epoll",
                )
            })?;

        self.listener = Some(listener);

        Ok(())
    }

    /// Get a copy of the `Waker` for this `Poll` instance
    pub fn waker(&self) -> Arc<Waker> {
        self.waker.clone()
    }

    pub fn poll(&mut self, events: &mut Events, timeout: Duration) -> Result<(), std::io::Error> {
        self.poll.poll(events, Some(timeout))
    }

    pub fn accept(&mut self) -> Result<Token, std::io::Error> {
        if let Some(ref mut listener) = self.listener {
            let stream = listener.accept()?;

            // disable Nagle's algorithm
            let _ = stream.set_nodelay(true);

            let session = Session::from(stream);

            self.add_session(session)
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "not listening",
            ))
        }
    }

    // Session methods

    /// Add a new session
    pub fn add_session(&mut self, session: Session) -> Result<Token, std::io::Error> {
        let s = self.sessions.vacant_entry();
        let token = Token(s.key());
        let mut session = TrackedSession {
            session,
            sender: None,
            token: None,
        };
        session.session.set_token(token);
        session.session.register(&self.poll)?;
        s.insert(session);
        Ok(token)
    }

    /// Close an existing session
    pub fn close_session(&mut self, token: Token) -> Result<(), std::io::Error> {
        let mut session = self.remove_session(token)?;
        trace!("closing session: {:?}", session.session);
        session.session.close();
        Ok(())
    }

    /// Remove a session from the poller and return it to the caller
    pub fn remove_session(&mut self, token: Token) -> Result<TrackedSession<S>, std::io::Error> {
        let mut session = self.take_session(token)?;
        trace!("removing session: {:?}", session.session);
        session.session.deregister(&self.poll)?;
        Ok(session)
    }

    pub fn get_mut_session(&mut self, token: Token) -> Result<&mut TrackedSession<S>, std::io::Error> {
        self.sessions
            .get_mut(token.0)
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::Other, "no such session"))
    }

    fn take_session(&mut self, token: Token) -> Result<TrackedSession<S>, std::io::Error> {
        if self.sessions.contains(token.0) {
            let session = self.sessions.remove(token.0);
            Ok(session)
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "no such session",
            ))
        }
    }

    pub fn reregister(&mut self, token: Token) {
        match token {
            LISTENER_TOKEN => {
                if let Some(ref mut listener) = self.listener {
                    if listener
                        .reregister(self.poll.registry(), LISTENER_TOKEN, Interest::READABLE)
                        .is_err()
                    {
                        warn!("reregister of listener failed, attempting to recover");
                        let _ = listener.deregister(self.poll.registry());
                        if listener
                            .register(self.poll.registry(), LISTENER_TOKEN, Interest::READABLE)
                            .is_err()
                        {
                            panic!("reregister of listener failed and was unrecoverable");
                        }
                    }
                }
            }
            WAKER_TOKEN => {
                trace!("reregister of waker token is not supported");
            }
            _ => {
                if let Some(session) = self.sessions.get_mut(token.0) {
                    trace!("reregistering session: {:?}", session.session);
                    if session.session.reregister(&self.poll).is_err() {
                        error!("failed to reregister session");
                        let _ = self.close_session(token);
                    }
                } else {
                    trace!("attempted to reregister non-existent session: {}", token.0);
                }
            }
        }
    }
}
