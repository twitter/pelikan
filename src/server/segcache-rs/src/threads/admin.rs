// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! The admin thread, which handles admin requests to return stats, get version
//! info, etc.

use crate::common::Queue;
use crate::common::Sender;
use crate::common::Signal;
use crate::event_loop::EventLoop;
use crate::protocol::admin::*;
use crate::session::TcpStream;
use crate::session::*;
use config::AdminConfig;
use mio::Events;
use mio::Interest;
use mio::Poll;
use mio::Token;
use slab::Slab;
use std::net::SocketAddr;
use std::time::Duration;

use metrics::Stat;

use rustcommon_fastmetrics::{Metric, Source};

use boring::ssl::{HandshakeError, MidHandshakeSslStream, Ssl, SslContext, SslStream};
use mio::event::Event;
use mio::net::TcpListener;
use strum::IntoEnumIterator;

use std::convert::TryInto;
use std::io::{Error, ErrorKind};

pub const LISTENER_TOKEN: usize = usize::MAX;

/// A `Admin` is used to bind to a given socket address and handle out-of-band
/// admin requests.
pub struct Admin {
    addr: SocketAddr,
    timeout: Duration,
    listener: TcpListener,
    nevent: usize,
    poll: Poll,
    ssl_context: Option<SslContext>,
    sessions: Slab<Session>,
    signal_queue: Queue<Signal>,
}

impl Admin {
    /// Creates a new `Admin` event loop.
    pub fn new(config: &AdminConfig, ssl_context: Option<SslContext>) -> Result<Self, Error> {
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

        let ssl_context = if config.use_tls() { ssl_context } else { None };

        let timeout = std::time::Duration::from_millis(config.timeout() as u64);

        let nevent = config.nevent();

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

        let signal_queue = Queue::new(128);

        Ok(Self {
            addr,
            listener,
            nevent,
            poll,
            ssl_context,
            sessions,
            signal_queue,
            timeout,
        })
    }

    /// Adds a new fully established TLS session
    fn add_established_tls_session(&mut self, addr: SocketAddr, stream: SslStream<TcpStream>) {
        let mut session = Session::tls(addr, stream);
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
        let s = self.sessions.vacant_entry();
        let token = s.key();
        session.set_token(Token(token));
        if session.register(&self.poll).is_ok() {
            s.insert(session);
        } else {
            increment_counter!(&Stat::TcpAcceptEx);
        }
    }

    /// Repeatedly call accept on the listener
    fn do_accept(&mut self) {
        while let Ok((stream, addr)) = self.listener.accept() {
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

    /// Handle an event on an existing session
    fn handle_session_event(&mut self, event: &Event) {
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
                        return;
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

    /// Runs the `Admin` in a loop, accepting new sessions for the admin
    /// listener and handling events on existing sessions.
    pub fn run(&mut self) {
        info!("running admin on: {}", self.addr);

        let mut events = Events::with_capacity(self.nevent);
        let timeout = Some(self.timeout);

        // run in a loop, accepting new sessions and events on existing sessions
        loop {
            increment_counter!(&Stat::AdminEventLoop);

            if self.poll.poll(&mut events, timeout).is_err() {
                error!("Error polling");
            }

            increment_counter_by!(
                &Stat::AdminEventTotal,
                events.iter().count().try_into().unwrap(),
            );

            // handle all events
            for event in events.iter() {
                if event.token() == Token(LISTENER_TOKEN) {
                    self.do_accept();
                } else {
                    self.handle_session_event(event);
                }
            }

            // poll queue to receive new signals
            #[allow(clippy::never_loop)]
            while let Ok(signal) = self.signal_queue.try_recv() {
                match signal {
                    Signal::Shutdown => {
                        return;
                    }
                }
            }

            self.get_rusage();
        }
    }

    /// Returns a `SyncSender` which can be used to send `Message`s to the
    /// `Admin` component.
    pub fn signal_sender(&self) -> Sender<Signal> {
        self.signal_queue.sender()
    }

    // TODO(bmartin): move this into a common module, should be shared with
    // other backends
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
            set_gauge!(&Stat::RuMaxrss, rusage.ru_maxrss as i64);
            set_gauge!(&Stat::RuIxrss, rusage.ru_ixrss as i64);
            set_gauge!(&Stat::RuIdrss, rusage.ru_idrss as i64);
            set_gauge!(&Stat::RuIsrss, rusage.ru_isrss as i64);
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

    fn handle_data(&mut self, token: Token) -> Result<(), ()> {
        trace!("handling request for admin session: {}", token.0);
        if let Some(session) = self.sessions.get_mut(token.0) {
            loop {
                // TODO(bmartin): buffer should allow us to check remaining
                // write capacity.
                if session.write_pending() > MIN_BUFFER_SIZE {
                    // if the write buffer is over-full, skip processing
                    break;
                }
                match parse(&mut session.read_buffer) {
                    // match session.read_buffer.fill_buf() {
                    Ok(request) => match request {
                        Request::Stats => {
                            increment_counter!(&Stat::AdminRequestParse);
                            let mut data = Vec::new();
                            for metric in Stat::iter() {
                                match metric.source() {
                                    Source::Gauge => {
                                        data.push(format!(
                                            "STAT {} {}\r\n",
                                            metric,
                                            get_gauge!(&metric).unwrap_or(0)
                                        ));
                                    }
                                    Source::Counter => {
                                        data.push(format!(
                                            "STAT {} {}\r\n",
                                            metric,
                                            get_counter!(&metric).unwrap_or(0)
                                        ));
                                    }
                                }
                            }
                            data.sort();
                            for line in data {
                                session.write_buffer.extend(line.as_bytes());
                            }
                            session.write_buffer.extend(b"END\r\n");
                            increment_counter!(&Stat::AdminResponseCompose);
                        }
                        Request::Quit => {
                            self.close(token);
                            return Ok(());
                        }
                        Request::Version => {
                            session.write_buffer.extend(
                                format!("VERSION {}\r\n", env!("CARGO_PKG_VERSION")).as_bytes(),
                            );
                            increment_counter!(&Stat::AdminResponseCompose);
                        }
                    },
                    Err(ParseError::Incomplete) => {
                        break;
                    }
                    Err(_) => {
                        self.handle_error(token);
                        return Err(());
                    }
                }
            }
        } else {
            // no session for the token
            trace!(
                "attempted to handle data for non-existent session: {}",
                token.0
            );
            return Ok(());
        }
        self.reregister(token);
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
