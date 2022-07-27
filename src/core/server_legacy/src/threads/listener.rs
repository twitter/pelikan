// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! The server thread which accepts new connections, handles TLS handshaking,
//! and sends established sessions to the worker thread(s).

use ::net::event::Source;
use crate::LISTENER_TOKEN;
use crate::WAKER_TOKEN;
use slab::Slab;
use config::TlsConfig;
use common::ssl::tls_acceptor;
use ::net::event::Event;
use session_common::Session;
// use net::Stream;
use ::net::*;
// use core::marker::PhantomData;
// use net::TcpStream;
// use super::EventLoop;
// use crate::poll::{Poll, LISTENER_TOKEN, WAKER_TOKEN};
use crate::*;
use common::signal::Signal;
// use common::ssl::{HandshakeError, MidHandshakeSslStream, Ssl, SslContext, SslStream};
use config::ServerConfig;
// use net::event::Event;
// use net::Events;
// use net::Token;
use queues::*;
// use session_legacy::{Session, TcpStream};
// use session_common::ServerSession;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use std::io::ErrorKind;

counter!(SERVER_EVENT_ERROR);
counter!(SERVER_EVENT_WRITE);
counter!(SERVER_EVENT_READ);
counter!(SERVER_EVENT_LOOP);
counter!(SERVER_EVENT_TOTAL);

pub struct ListenerBuilder {
    listener: ::net::Listener,
    poll: Poll,
    sessions: Slab<Session>,
    waker: Arc<Waker>,


    addr: SocketAddr,
    max_buffer_size: usize,
    nevent: usize,
    // poll: Poll,
    timeout: Duration,
    // parser: Parser,
    // _request: PhantomData<Request>,
    // _response: PhantomData<Response>,
}

impl ListenerBuilder {
    /// Creates a new `Listener` from a `ServerConfig` and an optional
    /// `SslContext`.
    pub fn new<T: ServerConfig + TlsConfig>(
        config: &T,
        max_buffer_size: usize,
    ) -> Result<Self, std::io::Error> {
        let tls_config = config.tls();
        let config = config.server();

        let addr = config.socket_addr().map_err(|e| {
            error!("{}", e);
            std::io::Error::new(std::io::ErrorKind::Other, "Bad listen address")
        })?;

        let tcp_listener = TcpListener::bind(addr)?;

        let listener = if let Some(tls_acceptor) = tls_acceptor(tls_config)? {
            ::net::Listener::from((tcp_listener, tls_acceptor))
        } else {
            ::net::Listener::from(tcp_listener)
        };

        let mut poll = Poll::new()?;
        listener.register(poll.registry(), LISTENER_TOKEN, Interest::READABLE)?;

        let waker = Arc::new(Waker::new(poll.registry(), WAKER_TOKEN).unwrap());

        let nevent = config.nevent();
        let timeout = Duration::from_millis(config.timeout() as u64);

        let sessions = Slab::new();

        Ok(Self {
            listener,
            sessions,
            waker,

            addr,
            nevent,
            poll,
            timeout,
            max_buffer_size,
        })
    }

    pub fn waker(&self) -> Arc<Waker> {
        self.waker.clone()
    }

    pub fn build(
        self,
        signal_queue: Queues<(), Signal>,
        session_queue: Queues<Session, ()>,
    ) -> Listener {
        Listener {
            listener: self.listener,
            poll: self.poll,
            sessions: Slab::new(),
            waker: self.waker,

            addr: self.addr,
            max_buffer_size: self.max_buffer_size,
            nevent: self.nevent,
            // poll: self.poll,
            // ssl_context: self.ssl_context,
            timeout: self.timeout,
            signal_queue,
            session_queue,
        }
    }
}

pub struct Listener {
    listener: ::net::Listener,
    poll: Poll,
    sessions: Slab<Session>,
    waker: Arc<Waker>,

    addr: SocketAddr,
    max_buffer_size: usize,
    nevent: usize,
    // poll: Poll,
    // ssl_context: Option<SslContext>,
    timeout: Duration,
    signal_queue: Queues<(), Signal>,
    session_queue: Queues<Session, ()>,
}

impl Listener {
    /// Call accept one time
    // TODO(bmartin): splitting accept and negotiation into separate threads
    // would allow us to handle TLS handshake with multiple threads and avoid
    // the overhead of re-registering the listener after each accept.
    fn do_accept(&mut self) {
        if let Ok(session) = self.listener.accept().map(|s| Session::from(s)) {
            if session.is_handshaking() {
                let s = self.sessions.vacant_entry();
                session.register(self.poll.registry(), Token(s.key()), session.interest());
                s.insert(session);
            } else {
                self.session_queue.try_send_any(session);
            }
        }

        self.listener.reregister(self.poll.registry(), LISTENER_TOKEN, Interest::READABLE);
    }

    // /// Adds a new fully established TLS session
    // fn add_established_tls_session(&mut self, stream: SslStream<TcpStream>) {
    //     let session = ServerSession::from(stream)
    //         Session::tls_with_capacity(stream, crate::DEFAULT_BUFFER_SIZE, self.max_buffer_size);
    //     trace!("accepted new session: {:?}", session);
    //     if self.session_queue.try_send_any(session).is_err() {
    //         error!("error sending session to worker");
    //         TCP_ACCEPT_EX.increment();
    //     }
    // }

    // /// Adds a new TLS session that requires further handshaking
    // fn add_handshaking_tls_session(&mut self, stream: MidHandshakeSslStream<TcpStream>) {
    //     let session = Session::handshaking_with_capacity(
    //         stream,
    //         crate::DEFAULT_BUFFER_SIZE,
    //         self.max_buffer_size,
    //     );
    //     if self.poll.add_session(session).is_err() {
    //         error!("failed to register handshaking TLS session with epoll");
    //         TCP_ACCEPT_EX.increment();
    //     }
    // }

    // /// Adds a new plain (non-TLS) session
    // fn add_plain_session(&mut self, stream: TcpStream) {
    //     let session =
    //         Session::plain_with_capacity(stream, crate::DEFAULT_BUFFER_SIZE, self.max_buffer_size);
    //     trace!("accepted new session: {:?}", session);
    //     if self.session_queue.try_send_any(session).is_err() {
    //         error!("error sending session to worker");
    //         TCP_ACCEPT_EX.increment();
    //     }
    // }

    /// Handle errors for the `Session` with the `Token` by logging a message
    /// and closing the session.
    fn handle_error(&mut self, token: Token) {
        if let Some(session) = self.sessions.get_mut(token.0) {
            trace!("handling error for session: {:?}", session);
            let _ = session.flush();
            let _ = self.sessions.remove(token.0);
        } else {
            trace!(
                "attempted to handle error for non-existent session: {}",
                token.0
            )
        }
    }

    /// Handle an event on an existing session
    fn handle_session_event(&mut self, event: &Event) {
        let token = event.token();

        // handle error events first
        if event.is_error() {
            SERVER_EVENT_ERROR.increment();
            self.handle_error(token);
        }

        // handle write events before read events to reduce write
        // buffer growth if there is also a readable event
        if event.is_writable() {
            SERVER_EVENT_WRITE.increment();
        //     self.do_write(token);
        }

        // read events are handled last
        if event.is_readable() {
            SERVER_EVENT_READ.increment();
            let _ = self.do_read(token);
        }

        if let Some(session) = self.sessions.get_mut(token.0) {
            if session.do_handshake().is_ok() {
                trace!("handshake complete for session: {:?}", session);
                let session = self.sessions.remove(token.0);
                if self.session_queue.try_send_any(session).is_err() {
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

    /// Runs the `Listener` in a loop, accepting new sessions and moving them to
    /// a worker queue.
    pub fn run(&mut self) {
        info!("running server on: {}", self.addr);

        let mut events = Events::with_capacity(self.nevent);

        // repeatedly run accepting new connections and moving them to the worker
        loop {
            SERVER_EVENT_LOOP.increment();
            if self.poll.poll(&mut events, Some(self.timeout)).is_err() {
                error!("Error polling server");
            }
            SERVER_EVENT_TOTAL.add(events.iter().count() as _);

            // handle all events
            for event in events.iter() {
                match event.token() {
                    LISTENER_TOKEN => {
                        self.do_accept();
                    }
                    WAKER_TOKEN => {
                        while let Some(signal) =
                            self.signal_queue.try_recv().map(|v| v.into_inner())
                        {
                            match signal {
                                Signal::FlushAll => {}
                                Signal::Shutdown => {
                                    return;
                                }
                            }
                        }
                    }
                    _ => {
                        self.handle_session_event(event);
                    }
                }
            }

            let _ = self.session_queue.wake();
        }
    }
}

// impl EventLoop for Listener {
//     fn handle_data(&mut self, _token: Token) -> Result<(), std::io::Error> {
//         Ok(())
//     }

//     fn poll(&mut self) -> &mut Poll {
//         &mut self.poll
//     }
// }
