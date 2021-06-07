// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! The server thread which accepts new connections, handles TLS handshaking,
//! and sends established sessions to the worker thread(s).

use super::EventLoop;
use crate::poll::{Poll, LISTENER_TOKEN, WAKER_TOKEN};
use boring::ssl::{HandshakeError, MidHandshakeSslStream, Ssl, SslContext, SslStream};
use common::signal::Signal;
use config::ServerConfig;
use metrics::Stat;
use mio::event::Event;
use mio::Events;
use mio::Token;
use queues::*;
use session::{Session, TcpStream};
use std::convert::TryInto;
use std::io::ErrorKind;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

/// A `Server` is used to bind to a given socket address and accept new
/// sessions. Fully negotiated sessions are then moved into a `Worker` thread
/// over a queue.
pub struct Listener {
    addr: SocketAddr,
    nevent: usize,
    poll: Poll,
    session_queue: QueuePairs<Session, ()>,
    ssl_context: Option<SslContext>,
    signal_queue: QueuePairs<(), Signal>,
    timeout: Duration,
}

impl Listener {
    /// Creates a new `Listener` from a `ServerConfig` and an optional
    /// `SslContext`.
    pub fn new(
        config: &ServerConfig,
        ssl_context: Option<SslContext>,
    ) -> Result<Self, std::io::Error> {
        let addr = config.socket_addr().map_err(|e| {
            error!("{}", e);
            std::io::Error::new(std::io::ErrorKind::Other, "Bad listen address")
        })?;
        let mut poll = Poll::new().map_err(|e| {
            error!("{}", e);
            std::io::Error::new(std::io::ErrorKind::Other, "Failed to create epoll instance")
        })?;

        poll.bind(addr)?;

        let nevent = config.nevent();
        let timeout = Duration::from_millis(config.timeout() as u64);

        let signal_queue = QueuePairs::new(Some(poll.waker()));
        let session_queue = QueuePairs::new(Some(poll.waker()));

        Ok(Self {
            addr,
            nevent,
            poll,
            session_queue,
            ssl_context,
            signal_queue,
            timeout,
        })
    }

    /// Repeatedly call accept on the listener
    fn do_accept(&mut self) {
        loop {
            match self.poll.accept() {
                Ok((stream, _)) => {
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
                Err(e) => {
                    if e.kind() == ErrorKind::WouldBlock {
                        break;
                    }
                }
            }
        }
    }

    /// Adds a new fully established TLS session
    fn add_established_tls_session(&mut self, stream: SslStream<TcpStream>) {
        let session = Session::tls_with_capacity(stream, crate::DEFAULT_BUFFER_SIZE);
        trace!("accepted new session: {:?}", session.peer_addr());
        if self.session_queue.send_rr(session).is_err() {
            error!("error sending session to worker");
            increment_counter!(&Stat::TcpAcceptEx);
        }
    }

    /// Adds a new TLS session that requires further handshaking
    fn add_handshaking_tls_session(&mut self, stream: MidHandshakeSslStream<TcpStream>) {
        let session = Session::handshaking_with_capacity(stream, crate::DEFAULT_BUFFER_SIZE);
        if self.poll.add_session(session).is_err() {
            increment_counter!(&Stat::TcpAcceptEx);
        }
    }

    /// Adds a new plain (non-TLS) session
    fn add_plain_session(&mut self, stream: TcpStream) {
        let session = Session::plain_with_capacity(stream, crate::DEFAULT_BUFFER_SIZE);
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

        if let Ok(session) = self.poll.get_mut_session(token) {
            if session.do_handshake().is_ok() {
                if let Ok(session) = self.poll.remove_session(token) {
                    if self.session_queue.send_rr(session).is_err() {
                        error!("error sending session to worker");
                        increment_counter!(&Stat::TcpAcceptEx);
                    }
                } else {
                    error!("error removing session from poller");
                    increment_counter!(&Stat::TcpAcceptEx);
                }
            }
        }
    }

    /// Runs the `Listener` in a loop, accepting new sessions and moving them to
    /// a worker queue.
    pub fn run(&mut self) {
        info!("running server on: {}", self.addr);

        let mut events = Events::with_capacity(self.nevent);

        // repeatedly run accepting new connections and moving them to the worker
        loop {
            increment_counter!(&Stat::ServerEventLoop);
            if self.poll.poll(&mut events, self.timeout).is_err() {
                error!("Error polling server");
            }
            increment_counter_by!(
                &Stat::ServerEventTotal,
                events.iter().count().try_into().unwrap(),
            );

            // handle all events
            for event in events.iter() {
                match event.token() {
                    LISTENER_TOKEN => {
                        self.do_accept();
                    }
                    WAKER_TOKEN =>
                    {
                        #[allow(clippy::never_loop)]
                        while let Ok(signal) = self.signal_queue.recv_from(0) {
                            match signal {
                                Signal::Shutdown => {
                                    return;
                                }
                            }
                        }
                    }
                    _ => {
                        self.handle_session_event(&event);
                    }
                }
            }
        }
    }

    /// Returns a copy of the `Waker` for this thread which can be used to
    /// signal that there are pending messages on a queue.
    pub fn waker(&self) -> Arc<Waker> {
        self.poll.waker()
    }

    /// Register a `Worker`'s `Session` queue with this thread. Established
    /// sessions will be sent to a worker over its `QueuePair`.
    pub fn add_session_queue(&mut self, queue: QueuePair<Session, ()>) {
        self.session_queue.add_pair(queue);
    }

    /// Get a `QueuePair` for sending `Signal`s to this thread.
    pub fn signal_queue(&mut self) -> QueuePair<Signal, ()> {
        self.signal_queue.new_pair(128, None)
    }
}

impl EventLoop for Listener {
    fn handle_data(&mut self, _token: Token) -> Result<(), std::io::Error> {
        Ok(())
    }

    fn poll(&mut self) -> &mut Poll {
        &mut self.poll
    }
}
