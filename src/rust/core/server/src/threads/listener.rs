// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! The server thread which accepts new connections, handles TLS handshaking,
//! and sends established sessions to the worker thread(s).

use super::EventLoop;
use common::signal::Signal;
use config::ServerConfig;
use mio::Events;
use mio::Interest;
use mio::Poll;
use mio::Token;
use queues::*;
use session::{Session, TcpStream};
use slab::Slab;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use boring::ssl::{HandshakeError, MidHandshakeSslStream, Ssl, SslContext, SslStream};
use metrics::Stat;
use mio::event::Event;
use mio::net::TcpListener;

use std::convert::TryInto;

pub const WAKER_TOKEN: usize = usize::MAX - 1;
pub const LISTENER_TOKEN: usize = usize::MAX;

/// A `Server` is used to bind to a given socket address and accept new
/// sessions. These sessions are moved onto a MPSC queue, where they can be
/// handled by a `Worker`.
pub struct Listener {
    addr: SocketAddr,
    listener: TcpListener,
    nevent: usize,
    poll: Poll,
    session_queue: MultiQueuePair<Session, ()>,
    next_sender: usize,
    ssl_context: Option<SslContext>,
    sessions: Slab<Session>,
    signal_queue: MultiQueuePair<(), Signal>,
    timeout: Duration,
    waker: Arc<Waker>,
}

impl Listener {
    /// Creates a new `Listener` that will bind to a given `addr` and push new
    /// `Session`s over the `sender`
    pub fn new(
        config: &ServerConfig,
        ssl_context: Option<SslContext>,
    ) -> Result<Self, std::io::Error> {
        let addr = config.socket_addr().map_err(|e| {
            error!("{}", e);
            std::io::Error::new(std::io::ErrorKind::Other, "Bad listen address")
        })?;
        let mut listener = TcpListener::bind(addr).map_err(|e| {
            error!("{}", e);
            std::io::Error::new(std::io::ErrorKind::Other, "Failed to start tcp listener")
        })?;
        let poll = Poll::new().map_err(|e| {
            error!("{}", e);
            std::io::Error::new(std::io::ErrorKind::Other, "Failed to create epoll instance")
        })?;

        let nevent = config.nevent();
        let timeout = Duration::from_millis(config.timeout() as u64);

        let waker = Arc::new(Waker::new(poll.registry(), Token(WAKER_TOKEN)).unwrap());

        let signal_queue = MultiQueuePair::new(Some(waker.clone()));
        let session_queue = MultiQueuePair::new(Some(waker.clone()));

        // register listener to event loop
        poll.registry()
            .register(&mut listener, Token(LISTENER_TOKEN), Interest::READABLE)
            .map_err(|e| {
                error!("{}", e);
                std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Failed to register listener with epoll",
                )
            })?;

        let sessions = Slab::<Session>::new();

        Ok(Self {
            addr,
            listener,
            nevent,
            poll,
            sessions,
            session_queue,
            signal_queue,
            next_sender: 0,
            ssl_context,
            timeout,
            waker,
        })
    }

    /// Repeatedly call accept on the listener
    fn do_accept(&mut self) {
        while let Ok((stream, _)) = self.listener.accept() {
            // disable Nagle's algorithm
            let _ = stream.set_nodelay(true);

            let stream = TcpStream::from(stream);

            // handle TLS if it is configured
            if let Some(ssl_context) = &self.ssl_context {
                match Ssl::new(&ssl_context).map(|v| v.accept(stream)) {
                    // handle case where we have a fully-negotiated
                    // TLS stream on accept()
                    Ok(Ok(tls_stream)) => {
                        self.add_established_tls_session(tls_stream);
                    }
                    // handle case where further negotiation is
                    // needed
                    Ok(Err(HandshakeError::WouldBlock(tls_stream))) => {
                        self.add_handshaking_tls_session(tls_stream);
                    }
                    // some other error has occurred and we drop the
                    // stream
                    Ok(Err(_)) | Err(_) => {
                        increment_counter!(&Stat::TcpAcceptEx);
                    }
                }
            } else {
                self.add_plain_session(stream);
            };
        }
    }

    /// Adds a new fully established TLS session
    fn add_established_tls_session(&mut self, stream: SslStream<TcpStream>) {
        let mut session = Session::tls_with_capacity(stream, crate::DEFAULT_BUFFER_SIZE);
        trace!("accepted new session: {:?}", session.peer_addr());
        let mut success = false;
        if self.session_queue.send_rr(session).is_err() {
            error!("error sending session to worker");
            increment_counter!(&Stat::TcpAcceptEx);
        }
    }

    /// Adds a new TLS session that requires further handshaking
    fn add_handshaking_tls_session(&mut self, stream: MidHandshakeSslStream<TcpStream>) {
        let mut session = Session::handshaking_with_capacity(stream, crate::DEFAULT_BUFFER_SIZE);
        let s = self.sessions.vacant_entry();
        let token = s.key();
        session.set_token(Token(token));
        if session.register(&self.poll).is_ok() {
            s.insert(session);
        } else {
            increment_counter!(&Stat::TcpAcceptEx);
        }
    }

    /// Adds a new plain (non-TLS) session
    fn add_plain_session(&mut self, stream: TcpStream) {
        let mut session = Session::plain_with_capacity(stream, crate::DEFAULT_BUFFER_SIZE);
        trace!("accepted new session: {:?}", session.peer_addr());
        if self.session_queue.send_rr(session).is_err() {
            error!("error sending session to worker");
            increment_counter!(&Stat::TcpAcceptEx);
        }
    }

    /// Handle an event on an existing session
    fn handle_session_event(&mut self, event: &Event) {
        let token = event.token();
        trace!("got event for session: {}", token.0);

        // handle error events first
        if event.is_error() {
            increment_counter!(&Stat::ServerEventError);
            self.handle_error(token);
        }

        // handle write events before read events to reduce write
        // buffer growth if there is also a readable event
        if event.is_writable() {
            increment_counter!(&Stat::ServerEventWrite);
            self.do_write(token);
        }

        // read events are handled last
        if event.is_readable() {
            increment_counter!(&Stat::ServerEventRead);
            let _ = self.do_read(token);
        }

        if let Some(session) = self.sessions.get_mut(token.0) {
            if session.do_handshake().is_ok() {
                let mut session = self.sessions.remove(token.0);
                let _ = session.deregister(&self.poll);
                if self.session_queue.send_rr(session).is_err() {
                    error!("error sending session to worker");
                    increment_counter!(&Stat::TcpAcceptEx);
                }
            }
        }
    }

    /// Runs the `Server` in a loop, accepting new sessions and moving them to
    /// the queue
    pub fn run(&mut self) {
        info!("running server on: {}", self.addr);

        let mut events = Events::with_capacity(self.nevent);
        let timeout = Some(self.timeout);

        // repeatedly run accepting new connections and moving them to the worker
        loop {
            increment_counter!(&Stat::ServerEventLoop);
            if self.poll.poll(&mut events, timeout).is_err() {
                error!("Error polling server");
            }
            increment_counter_by!(
                &Stat::ServerEventTotal,
                events.iter().count().try_into().unwrap(),
            );

            // handle all events
            for event in events.iter() {
                if event.token() == Token(LISTENER_TOKEN) {
                    self.do_accept();
                } else {
                    self.handle_session_event(&event);
                }
            }

            // poll queue to receive new signals
            // #[allow(clippy::never_loop)]
            // while let Ok(signal) = self.signal_queue.try_recv() {
            //     match signal {
            //         Signal::Shutdown => {
            //             return;
            //         }
            //     }
            // }
        }
    }

    pub fn waker(&self) -> Arc<Waker> {
        self.waker.clone()
    }

    pub fn add_session_queue(&mut self, queue: QueuePair<Session, ()>) {
        self.session_queue.register_queue_pair(queue);
    }
}

impl EventLoop for Listener {
    fn get_mut_session(&mut self, token: Token) -> Option<&mut Session> {
        self.sessions.get_mut(token.0)
    }

    fn handle_data(&mut self, _token: Token) -> Result<(), ()> {
        Ok(())
    }

    fn take_session(&mut self, token: Token) -> Option<Session> {
        if self.sessions.contains(token.0) {
            let session = self.sessions.remove(token.0);
            Some(session)
        } else {
            None
        }
    }

    /// Reregister the session given its token
    fn reregister(&mut self, token: Token) {
        trace!("reregistering session: {}", token.0);
        if let Some(session) = self.sessions.get_mut(token.0) {
            if session.reregister(&self.poll).is_err() {
                error!("Failed to reregister");
                self.close(token);
            }
        } else {
            trace!("attempted to reregister non-existent session: {}", token.0);
        }
    }

    fn poll(&self) -> &Poll {
        &self.poll
    }
}
