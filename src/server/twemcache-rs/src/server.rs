// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::event_loop::EventLoop;
use crate::session::*;
use crate::*;

use boring::ssl::{HandshakeError, MidHandshakeSslStream, Ssl, SslContext, SslStream};
use crossbeam_channel::{Receiver, SendError, Sender};
use metrics::Stat;
use mio::event::Event;
use mio::net::TcpListener;

use std::convert::TryInto;

pub const LISTENER_TOKEN: usize = usize::MAX;

/// A `Server` is used to bind to a given socket address and accept new
/// sessions. These sessions are moved onto a MPSC queue, where they can be
/// handled by a `Worker`.
pub struct Server {
    addr: SocketAddr,
    config: Arc<Config>,
    listener: TcpListener,
    poll: Poll,
    senders: Vec<Sender<Session>>,
    next_sender: usize,
    ssl_context: Option<SslContext>,
    sessions: Slab<Session>,
    message_receiver: Receiver<Message>,
    message_sender: Sender<Message>,
}

impl Server {
    /// Creates a new `Server` that will bind to a given `addr` and push new
    /// `Session`s over the `sender`
    pub fn new(config: Arc<Config>, senders: Vec<Sender<Session>>) -> Result<Self, std::io::Error> {
        let addr = config.server().socket_addr().map_err(|e| {
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

        let ssl_context = crate::common::ssl_context(&config)?;

        let (message_sender, message_receiver) = crossbeam_channel::bounded(128);

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
            config,
            listener,
            poll,
            senders,
            next_sender: 0,
            ssl_context,
            sessions,
            message_sender,
            message_receiver,
        })
    }

    /// Repeatedly call accept on the listener
    fn do_accept(&mut self) {
        while let Ok((stream, addr)) = self.listener.accept() {
            // disable Nagle's algorithm
            let _ = stream.set_nodelay(true);

            let stream = TcpStream::from(stream);

            // handle TLS if it is configured
            if let Some(ssl_context) = &self.ssl_context {
                match Ssl::new(&ssl_context).map(|v| v.accept(stream)) {
                    // handle case where we have a fully-negotiated
                    // TLS stream on accept()
                    Ok(Ok(tls_stream)) => {
                        self.add_established_tls_session(addr, tls_stream);
                    }
                    // handle case where further negotiation is
                    // needed
                    Ok(Err(HandshakeError::WouldBlock(tls_stream))) => {
                        self.add_handshaking_tls_session(addr, tls_stream);
                    }
                    // some other error has occurred and we drop the
                    // stream
                    Ok(Err(_)) | Err(_) => {
                        increment_counter!(&Stat::TcpAcceptEx);
                    }
                }
            } else {
                self.add_plain_session(addr, stream);
            };
        }
    }

    /// Adds a new fully established TLS session
    fn add_established_tls_session(&mut self, addr: SocketAddr, stream: SslStream<TcpStream>) {
        let mut session = Session::tls(addr, stream);
        trace!("accepted new session: {}", addr);
        let mut success = false;
        for i in 0..self.senders.len() {
            let index = (self.next_sender + i) % self.senders.len();
            match self.senders[index].send(session) {
                Ok(_) => {
                    success = true;
                    self.next_sender = self.next_sender.wrapping_add(1);
                    break;
                }
                Err(SendError(s)) => {
                    session = s;
                }
            }
        }
        if !success {
            error!("error sending session to worker");
            increment_counter!(&Stat::TcpAcceptEx);
        }
    }

    /// Adds a new TLS session that requires further handshaking
    fn add_handshaking_tls_session(
        &mut self,
        addr: SocketAddr,
        stream: MidHandshakeSslStream<TcpStream>,
    ) {
        let mut session = Session::handshaking(addr, stream);
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
    fn add_plain_session(&mut self, addr: SocketAddr, stream: TcpStream) {
        let mut session = Session::plain(addr, stream);
        trace!("accepted new session: {}", addr);
        let mut success = false;
        for i in 0..self.senders.len() {
            let index = (self.next_sender + i) % self.senders.len();
            match self.senders[index].send(session) {
                Ok(_) => {
                    success = true;
                    self.next_sender = self.next_sender.wrapping_add(1);
                    break;
                }
                Err(SendError(s)) => {
                    session = s;
                }
            }
        }
        if !success {
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
                let mut success = false;
                for i in 0..self.senders.len() {
                    let index = (self.next_sender + i) % self.senders.len();
                    match self.senders[index].send(session) {
                        Ok(_) => {
                            success = true;
                            self.next_sender = self.next_sender.wrapping_add(1);
                            break;
                        }
                        Err(SendError(s)) => {
                            session = s;
                        }
                    }
                }
                if !success {
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

        let mut events = Events::with_capacity(self.config.server().nevent());
        let timeout = Some(std::time::Duration::from_millis(
            self.config.server().timeout() as u64,
        ));

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

            // poll queue to receive new messages
            #[allow(clippy::never_loop)]
            while let Ok(message) = self.message_receiver.try_recv() {
                match message {
                    Message::Shutdown => {
                        return;
                    }
                }
            }
        }
    }

    pub fn message_sender(&self) -> Sender<Message> {
        self.message_sender.clone()
    }
}

impl EventLoop for Server {
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
