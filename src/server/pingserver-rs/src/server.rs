// Copyright 2020 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::event_loop::EventLoop;
use crate::session::*;
use crate::*;

use mio::net::TcpListener;

use std::convert::TryInto;

/// A `Server` is used to bind to a given socket address and accept new
/// sessions. These sessions are moved onto a MPSC queue, where they can be
/// handled by a `Worker`.
pub struct Server {
    addr: SocketAddr,
    config: Arc<PingserverConfig>,
    listener: TcpListener,
    poll: Poll,
    sender: SyncSender<Session>,
    tls_config: Option<Arc<rustls::ServerConfig>>,
    sessions: Slab<Session>,
    metrics: Arc<Metrics<AtomicU64, AtomicU64>>,
    message_receiver: Receiver<Message>,
    message_sender: SyncSender<Message>,
}

pub const LISTENER_TOKEN: usize = usize::MAX;

impl Server {
    /// Creates a new `Server` that will bind to a given `addr` and push new
    /// `Session`s over the `sender`
    pub fn new(
        config: Arc<PingserverConfig>,
        metrics: Arc<Metrics<AtomicU64, AtomicU64>>,
        sender: SyncSender<Session>,
    ) -> Result<Self, std::io::Error> {
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

        let tls_config = crate::common::load_tls_config(&config)?;

        let (message_sender, message_receiver) = sync_channel(128);

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
            sender,
            tls_config,
            sessions,
            metrics,
            message_sender,
            message_receiver,
        })
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
            let _ = self.metrics.increment_counter(&Stat::ServerEventLoop, 1);
            if self.poll.poll(&mut events, timeout).is_err() {
                error!("Error polling server");
            }
            let _ = self.metrics.increment_counter(
                &Stat::ServerEventTotal,
                events.iter().count().try_into().unwrap(),
            );

            // handle all events
            for event in events.iter() {
                if event.token() == Token(LISTENER_TOKEN) {
                    while let Ok((stream, addr)) = self.listener.accept() {
                        if let Some(tls_config) = &self.tls_config {
                            let mut session = Session::new(
                                addr,
                                stream,
                                State::Handshaking,
                                Some(rustls::ServerSession::new(&tls_config)),
                                self.metrics.clone(),
                            );
                            let s = self.sessions.vacant_entry();
                            let token = s.key();
                            session.set_token(Token(token));
                            if session.register(&self.poll).is_ok() {
                                s.insert(session);
                            } else {
                                let _ = self.metrics().increment_counter(&Stat::TcpAcceptEx, 1);
                            }
                        } else {
                            let session = Session::new(
                                addr,
                                stream,
                                State::Established,
                                None,
                                self.metrics.clone(),
                            );
                            trace!("accepted new session: {}", addr);
                            if self.sender.send(session).is_err() {
                                error!("error sending session to worker");
                                let _ = self.metrics().increment_counter(&Stat::TcpAcceptEx, 1);
                            }
                        };
                    }
                } else {
                    let token = event.token();
                    trace!("got event for session: {}", token.0);

                    if event.is_error() {
                        let _ = self.metrics().increment_counter(&Stat::ServerEventError, 1);
                        self.handle_error(token);
                    }

                    if event.is_readable() {
                        let _ = self.metrics.increment_counter(&Stat::ServerEventRead, 1);
                        let _ = self.do_read(token);
                    }

                    if event.is_writable() {
                        let _ = self.metrics.increment_counter(&Stat::ServerEventWrite, 1);
                        self.do_write(token);
                    }

                    if let Some(handshaking) =
                        self.sessions.get(token.0).map(|v| v.is_handshaking())
                    {
                        if !handshaking {
                            let mut session = self.sessions.remove(token.0);
                            let _ = session.deregister(&self.poll);
                            session.set_state(State::Established);
                            if self.sender.send(session).is_err() {
                                error!("error sending session to worker");
                                let _ = self.metrics().increment_counter(&Stat::TcpAcceptEx, 1);
                            }
                        }
                    }
                }
            }

            // poll queue to receive new messages
            while let Ok(message) = self.message_receiver.try_recv() {
                match message {
                    Message::Shutdown => {
                        return;
                    }
                }
            }
        }
    }

    pub fn message_sender(&self) -> SyncSender<Message> {
        self.message_sender.clone()
    }
}

impl EventLoop for Server {
    fn metrics(&self) -> &Arc<Metrics<AtomicU64, AtomicU64>> {
        &self.metrics
    }

    fn get_mut_session<'a>(&'a mut self, token: Token) -> Option<&'a mut Session> {
        self.sessions.get_mut(token.0)
    }

    fn handle_data(&mut self, _token: Token) {}

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
