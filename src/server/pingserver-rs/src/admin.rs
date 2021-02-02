// Copyright 2020 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::event_loop::EventLoop;
use crate::metrics::*;
use crate::session::*;
use crate::*;
use boring::ssl::{HandshakeError, Ssl, SslContext};

use mio::net::TcpListener;
use std::io::{BufRead, ErrorKind};
use strum::IntoEnumIterator;

/// A `Admin` is used to bind to a given socket address and handle out-of-band
/// admin requests.
pub struct Admin {
    addr: SocketAddr,
    config: Arc<PingserverConfig>,
    listener: TcpListener,
    poll: Poll,
    ssl_context: Option<SslContext>,
    sessions: Slab<Session>,
    message_receiver: Receiver<Message>,
    message_sender: SyncSender<Message>,
}

pub const LISTENER_TOKEN: usize = usize::MAX;

impl Admin {
    /// Creates a new `Admin` event loop.
    pub fn new(config: Arc<PingserverConfig>) -> Result<Self, std::io::Error> {
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
            increment_counter!(&Stat::AdminEventLoop);

            if self.poll.poll(&mut events, timeout).is_err() {
                error!("Error polling");
            }

            increment_counter_by!(&Stat::AdminEventTotal, events.iter().count() as u64,);

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
                                    Session::tls(addr, tls_stream)
                                }
                                Ok(Err(HandshakeError::WouldBlock(tls_stream))) => {
                                    // session needs additional handshaking
                                    Session::handshaking(addr, tls_stream)
                                }
                                Ok(Err(_)) | Err(_) => {
                                    // unrecoverable error
                                    increment_counter!(&Stat::TcpAcceptEx);
                                    continue;
                                }
                            }
                        } else {
                            // plaintext session
                            Session::plain(addr, stream)
                        };

                        trace!("accepted new session: {}", addr);
                        let s = self.sessions.vacant_entry();
                        let token = s.key();
                        session.set_token(Token(token));
                        if session.register(&self.poll).is_ok() {
                            s.insert(session);
                        } else {
                            increment_counter!(&Stat::TcpAcceptEx);
                        }
                    }
                } else {
                    // handle events on existing sessions
                    let token = event.token();
                    trace!("got event for admin session: {}", token.0);

                    // handle error events first
                    if event.is_error() {
                        increment_counter!(&Stat::AdminEventError);
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
                        increment_counter!(&Stat::AdminEventWrite);
                        self.do_write(token);
                    }

                    // read events are handled last
                    if event.is_readable() {
                        increment_counter!(&Stat::AdminEventRead);
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

            self.get_rusage();
        }
    }

    /// Returns a `SyncSender` which can be used to send `Message`s to the
    /// `Admin` component.
    pub fn message_sender(&self) -> SyncSender<Message> {
        self.message_sender.clone()
    }

    pub fn get_rusage(&self) {
        let mut rusage = libc::rusage {
            ru_utime: libc::timeval {
                tv_sec: 0,
                tv_usec: 0,
            },
            ru_stime: libc::timeval {
                tv_sec: 0,
                tv_usec: 0,
            },
            ru_maxrss: 0,
            ru_ixrss: 0,
            ru_idrss: 0,
            ru_isrss: 0,
            ru_minflt: 0,
            ru_majflt: 0,
            ru_nswap: 0,
            ru_inblock: 0,
            ru_oublock: 0,
            ru_msgsnd: 0,
            ru_msgrcv: 0,
            ru_nsignals: 0,
            ru_nvcsw: 0,
            ru_nivcsw: 0,
        };

        if unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut rusage) } == 0 {
            set_counter!(
                &Stat::RuUtime,
                rusage.ru_utime.tv_sec as u64 * 1000000000 + rusage.ru_utime.tv_usec as u64 * 1000,
            );
            set_counter!(
                &Stat::RuStime,
                rusage.ru_stime.tv_sec as u64 * 1000000000 + rusage.ru_stime.tv_usec as u64 * 1000,
            );
            set_gauge!(&Stat::RuMaxrss, rusage.ru_maxrss);
            set_gauge!(&Stat::RuIxrss, rusage.ru_ixrss);
            set_gauge!(&Stat::RuIdrss, rusage.ru_idrss);
            set_gauge!(&Stat::RuIsrss, rusage.ru_isrss);
            set_counter!(&Stat::RuMinflt, rusage.ru_minflt as u64);
            set_counter!(&Stat::RuMajflt, rusage.ru_majflt as u64);
            set_counter!(&Stat::RuNswap, rusage.ru_nswap as u64);
            set_counter!(&Stat::RuInblock, rusage.ru_inblock as u64);
            set_counter!(&Stat::RuOublock, rusage.ru_oublock as u64);
            set_counter!(&Stat::RuMsgsnd, rusage.ru_msgsnd as u64);
            set_counter!(&Stat::RuMsgrcv, rusage.ru_msgrcv as u64);
            set_counter!(&Stat::RuNsignals, rusage.ru_nsignals as u64);
            set_counter!(&Stat::RuNvcsw, rusage.ru_nvcsw as u64);
            set_counter!(&Stat::RuNivcsw, rusage.ru_nivcsw as u64);
        }
    }
}

impl EventLoop for Admin {
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
                            let _ = increment_counter!(&Stat::AdminRequestParse);
                            session.buffer().consume(7);
                            let mut data = Vec::new();
                            for metric in Stat::iter() {
                                match metric {
                                    Stat::Pid
                                    | Stat::RuMaxrss
                                    | Stat::RuIxrss
                                    | Stat::RuIdrss
                                    | Stat::RuIsrss => {
                                        data.push(format!(
                                            "STAT {} {}\r\n",
                                            metric,
                                            get_gauge!(&metric).unwrap_or(0)
                                        ));
                                    }
                                    _ => {
                                        data.push(format!(
                                            "STAT {} {}\r\n",
                                            metric,
                                            get_counter!(&metric).unwrap_or(0)
                                        ));
                                    }
                                }
                            }
                            data.sort();
                            let mut content = data.join("");
                            content += "\r\nEND\r\n";
                            if session.write(content.as_bytes()).is_err() {
                                // error writing
                                increment_counter!(&Stat::AdminResponseComposeEx);
                                self.handle_error(token);
                                return;
                            } else {
                                increment_counter!(&Stat::AdminResponseCompose);
                            }
                        } else {
                            // invalid command
                            debug!("error");
                            increment_counter!(&Stat::AdminRequestParseEx);
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
