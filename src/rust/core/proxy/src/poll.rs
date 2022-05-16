// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! This module provides common functionality for threads which are based on an
//! event loop.

use crate::TCP_ACCEPT_EX;
use common::ssl::*;
use mio::event::Source;
use mio::{Events, Interest, Token, Waker};
use session::{Session, TcpStream};
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

struct TcpListener {
    inner: mio::net::TcpListener,
    ssl_context: Option<SslContext>,
}

impl TcpListener {
    pub fn bind(addr: SocketAddr, tls_config: &dyn TlsConfig) -> Result<Self, std::io::Error> {
        let listener = mio::net::TcpListener::bind(addr).map_err(|e| {
            error!("{}", e);
            std::io::Error::new(std::io::ErrorKind::Other, "failed to start tcp listener")
        })?;

        let ssl_context = common::ssl::ssl_context(tls_config)?;

        Ok(Self {
            inner: listener,
            ssl_context,
        })
    }
}

pub struct Poll {
    listener: Option<TcpListener>,
    poll: mio::Poll,
    sessions: Slab<TrackedSession>,
    waker: Arc<Waker>,
}

pub struct TrackedSession {
    pub session: Session,
    pub sender: Option<usize>,
    pub token: Option<Token>,
}

impl Poll {
    /// Create a new `Poll` instance.
    pub fn new() -> Result<Self, std::io::Error> {
        let poll = mio::Poll::new().map_err(|e| {
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
        let mut listener = TcpListener::bind(addr, tls_config).map_err(|e| {
            error!("{}", e);
            std::io::Error::new(std::io::ErrorKind::Other, "failed to start tcp listener")
        })?;

        // register listener to event loop
        self.poll
            .registry()
            .register(&mut listener.inner, LISTENER_TOKEN, Interest::READABLE)
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
            let (stream, _addr) = listener.inner.accept()?;

            // disable Nagle's algorithm
            let _ = stream.set_nodelay(true);

            let stream = TcpStream::try_from(stream)?;

            let session = if let Some(ssl_context) = &listener.ssl_context {
                match Ssl::new(ssl_context).map(|v| v.accept(stream)) {
                    // handle case where we have a fully-negotiated
                    // TLS stream on accept()
                    Ok(Ok(stream)) => {
                        Session::tls_with_capacity(stream, SESSION_BUFFER_MIN, SESSION_BUFFER_MAX)
                    }
                    // handle case where further negotiation is
                    // needed
                    Ok(Err(HandshakeError::WouldBlock(stream))) => {
                        Session::handshaking_with_capacity(stream, SESSION_BUFFER_MIN, SESSION_BUFFER_MAX)
                    }
                    // some other error has occurred and we drop the
                    // stream
                    Ok(Err(e)) => {
                        error!("accept failed: {}", e);
                        TCP_ACCEPT_EX.increment();
                        return Err(std::io::Error::new(std::io::ErrorKind::Other, "accept failed"));
                    }
                    Err(e) => {
                        error!("accept failed: {}", e);
                        TCP_ACCEPT_EX.increment();
                        return Err(std::io::Error::new(std::io::ErrorKind::Other, "accept failed"));
                    }
                }
            } else {
                Session::plain_with_capacity(stream, SESSION_BUFFER_MIN, SESSION_BUFFER_MAX)
            };

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
    pub fn remove_session(&mut self, token: Token) -> Result<TrackedSession, std::io::Error> {
        let mut session = self.take_session(token)?;
        trace!("removing session: {:?}", session.session);
        session.session.deregister(&self.poll)?;
        Ok(session)
    }

    pub fn get_mut_session(&mut self, token: Token) -> Result<&mut TrackedSession, std::io::Error> {
        self.sessions
            .get_mut(token.0)
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::Other, "no such session"))
    }

    fn take_session(&mut self, token: Token) -> Result<TrackedSession, std::io::Error> {
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
                        .inner
                        .reregister(self.poll.registry(), LISTENER_TOKEN, Interest::READABLE)
                        .is_err()
                    {
                        warn!("reregister of listener failed, attempting to recover");
                        let _ = listener.inner.deregister(self.poll.registry());
                        if listener
                            .inner
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
