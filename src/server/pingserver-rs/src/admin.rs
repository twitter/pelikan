// Copyright 2020 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::event_loop::EventLoop;
use crate::session::*;
use crate::*;
use boring::ssl::{HandshakeError, Ssl, SslContext};

use mio::net::TcpListener;

use std::convert::TryInto;
use std::io::{BufRead, ErrorKind};

/// A `Admin` is used to bind to a given socket address and handle out-of-band
/// admin requests.
pub struct Admin {
    addr: SocketAddr,
    config: Arc<PingserverConfig>,
    listener: TcpListener,
    poll: Poll,
    ssl_context: Option<SslContext>,
    sessions: Slab<Session>,
    metrics: Arc<Metrics<AtomicU64, AtomicU64>>,
    message_receiver: Receiver<Message>,
    message_sender: SyncSender<Message>,
}

pub const LISTENER_TOKEN: usize = usize::MAX;

impl Admin {
    /// Creates a new `Admin` event loop.
    pub fn new(
        config: Arc<PingserverConfig>,
        metrics: Arc<Metrics<AtomicU64, AtomicU64>>,
    ) -> Result<Self, std::io::Error> {
        let addr = config.admin().socket_addr().map_err(|e| {
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

        let (message_sender, message_receiver) = sync_channel(128);

        Ok(Self {
            addr,
            config,
            listener,
            poll,
            ssl_context,
            sessions,
            metrics,
            message_sender,
            message_receiver,
        })
    }

    /// Runs the `Admin` in a loop, accepting new sessions for the admin
    /// listener and handling events on existing sessions.
    pub fn run(&mut self) {
        info!("running admin on: {}", self.addr);

        let mut events = Events::with_capacity(self.config.server().nevent());
        let timeout = Some(std::time::Duration::from_millis(
            self.config.server().timeout() as u64,
        ));

        // run in a loop, accepting new sessions and events on existing sessions
        loop {
            self.increment_count(&Stat::AdminEventLoop);

            if self.poll.poll(&mut events, timeout).is_err() {
                error!("Error polling");
            }

            self.increment_count_n(
                &Stat::AdminEventTotal,
                events.iter().count().try_into().unwrap(),
            );

            // handle all events
            for event in events.iter() {
                // handle new sessions
                if event.token() == Token(LISTENER_TOKEN) {
                    while let Ok((stream, addr)) = self.listener.accept() {
                        let mut session = if let Some(ssl_context) = &self.ssl_context {
                            // handle TLS if it is configured
                            match Ssl::new(&ssl_context).map(|v| v.accept(stream)) {
                                Ok(Ok(tls_stream)) => {
                                    // fully-negotiated session on accept()
                                    Session::tls(addr, tls_stream, self.metrics.clone())
                                }
                                Ok(Err(HandshakeError::WouldBlock(tls_stream))) => {
                                    // session needs additional handshaking
                                    Session::handshaking(
                                        addr,
                                        tls_stream,
                                        self.metrics.clone(),
                                    )
                                }
                                Ok(Err(_)) | Err(_) => {
                                    // unrecoverable error
                                    let _ = self.metrics().increment_counter(&Stat::TcpAcceptEx, 1);
                                    continue;
                                }
                            }
                        } else {
                            // plaintext session
                            Session::plain(addr, stream, self.metrics.clone())
                        };

                        trace!("accepted new session: {}", addr);
                        let s = self.sessions.vacant_entry();
                        let token = s.key();
                        session.set_token(Token(token));
                        if session.register(&self.poll).is_ok() {
                            s.insert(session);
                        } else {
                            let _ =
                                self.metrics().increment_counter(&Stat::TcpAcceptEx, 1);
                        }
                    }
                } else {
                    // handle events on existing sessions
                    let token = event.token();
                    trace!("got event for admin session: {}", token.0);

                    // handle error events first
                    if event.is_error() {
                        self.increment_count(&Stat::AdminEventError);
                        self.handle_error(token);
                    }

                    // handle handshaking
                    if let Some(session) = self.sessions.get_mut(token.0) {
                        if session.is_handshaking() {
                            if let Err(e) = session.do_handshake() {
                                if e.kind() == ErrorKind::WouldBlock {
                                    // the session is still handshaking
                                    continue;
                                } else {
                                    // some error occured while handshaking
                                    self.close(token);
                                }
                            }
                        }
                    }

                    // handle write events before read events to reduce write
                    // buffer growth if there is also a readable event
                    if event.is_writable() {
                        self.increment_count(&Stat::AdminEventWrite);
                        self.do_write(token);
                    }

                    // read events are handled last
                    if event.is_readable() {
                        self.increment_count(&Stat::AdminEventRead);
                        let _ = self.do_read(token);
                    };
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

    /// Returns a `SyncSender` which can be used to send `Message`s to the
    /// `Admin` component.
    pub fn message_sender(&self) -> SyncSender<Message> {
        self.message_sender.clone()
    }
}

impl EventLoop for Admin {
    fn metrics(&self) -> &Arc<Metrics<AtomicU64, AtomicU64>> {
        &self.metrics
    }

    fn get_mut_session(&mut self, token: Token) -> Option<&mut Session> {
        self.sessions.get_mut(token.0)
    }

    fn handle_data(&mut self, token: Token) {
        trace!("handling request for admin session: {}", token.0);
        if let Some(session) = self.sessions.get_mut(token.0) {
            loop {
                // TODO(bmartin): buffer should allow us to check remaining
                // write capacity.
                if session.buffer().write_pending() > (1024 - 6) {
                    // if the write buffer is over-full, skip processing
                    break;
                }
                match session.buffer().fill_buf() {
                    Ok(buf) => {
                        // TODO(bmartin): improve the request parsing here to
                        // match twemcache-rs
                        if buf.len() < 7 {
                            break;
                        } else if &buf[0..7] == b"STATS\r\n" || &buf[0..7] == b"stats\r\n" {
                            let _ = self.metrics.increment_counter(&Stat::AdminRequestParse, 1);
                            session.buffer().consume(7);
                            let snapshot = self.metrics.snapshot();
                            let mut data = Vec::new();
                            for (metric, value) in snapshot {
                                let label = metric.statistic().name();
                                if let Output::Reading = metric.output() {
                                    data.push(format!("STAT {} {}", label, value));
                                }
                            }
                            data.sort();
                            let mut content = data.join("\r\n");
                            content += "\r\nEND\r\n";
                            if session.write(content.as_bytes()).is_err() {
                                // error writing
                                let _ = self
                                    .metrics
                                    .increment_counter(&Stat::AdminResponseComposeEx, 1);
                                self.handle_error(token);
                                return;
                            } else {
                                let _ = self
                                    .metrics
                                    .increment_counter(&Stat::AdminResponseCompose, 1);
                            }
                        } else {
                            // invalid command
                            debug!("error");
                            self.increment_count(&Stat::AdminRequestParseEx);
                            self.handle_error(token);
                            return;
                        }
                    }
                    Err(e) => {
                        if e.kind() == std::io::ErrorKind::WouldBlock {
                            break;
                        } else {
                            // couldn't get buffer contents
                            debug!("error");
                            self.handle_error(token);
                            return;
                        }
                    }
                }
            }
        } else {
            // no session for the token
            trace!(
                "attempted to handle data for non-existent session: {}",
                token.0
            );
            return;
        }
        self.reregister(token);
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
