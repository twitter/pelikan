// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! The admin thread, which handles admin requests to return stats, get version
//! info, etc.

use super::EventLoop;
use crate::poll::{Poll, LISTENER_TOKEN, WAKER_TOKEN};
use boring::ssl::{HandshakeError, MidHandshakeSslStream, Ssl, SslContext, SslStream};
use common::signal::Signal;
use config::AdminConfig;
use logger::PelikanLogReceiver;
use metrics::Stat;
use mio::event::Event;
use mio::Events;
use mio::Token;
use protocol::admin::*;
use protocol::*;
use queues::QueuePair;
use queues::QueuePairs;
use rustcommon_fastmetrics::{Metric, Source};
use session::*;
use std::convert::TryInto;
use std::io::{BufRead, Write};
use std::io::{Error, ErrorKind};
use std::net::SocketAddr;
use std::time::Duration;
use strum::IntoEnumIterator;

/// A `Admin` is used to bind to a given socket address and handle out-of-band
/// admin requests.
pub struct Admin {
    addr: SocketAddr,
    timeout: Duration,
    nevent: usize,
    poll: Poll,
    ssl_context: Option<SslContext>,
    signal_queue: QueuePairs<(), Signal>,
    parser: AdminRequestParser,
    log_receiver: PelikanLogReceiver,
}

impl Admin {
    /// Creates a new `Admin` event loop.
    pub fn new(
        config: &AdminConfig,
        ssl_context: Option<SslContext>,
        log_receiver: PelikanLogReceiver,
    ) -> Result<Self, Error> {
        let addr = config.socket_addr().map_err(|e| {
            error!("{}", e);
            std::io::Error::new(std::io::ErrorKind::Other, "Bad listen address")
        })?;
        let mut poll = Poll::new().map_err(|e| {
            error!("{}", e);
            std::io::Error::new(std::io::ErrorKind::Other, "Failed to create epoll instance")
        })?;
        poll.bind(addr)?;

        let ssl_context = if config.use_tls() { ssl_context } else { None };

        let timeout = std::time::Duration::from_millis(config.timeout() as u64);

        let nevent = config.nevent();

        let signal_queue = QueuePairs::new(Some(poll.waker()));

        Ok(Self {
            addr,
            timeout,
            nevent,
            poll,
            ssl_context,
            signal_queue,
            parser: AdminRequestParser::new(),
            log_receiver,
        })
    }

    /// Adds a new fully established TLS session
    fn add_established_tls_session(&mut self, stream: SslStream<TcpStream>) {
        let session = Session::tls_with_capacity(
            stream,
            crate::DEFAULT_BUFFER_SIZE,
            crate::ADMIN_MAX_BUFFER_SIZE,
        );
        if self.poll.add_session(session).is_err() {
            increment_counter!(&Stat::TcpAcceptEx);
        }
    }

    /// Adds a new TLS session that requires further handshaking
    fn add_handshaking_tls_session(&mut self, stream: MidHandshakeSslStream<TcpStream>) {
        let session = Session::handshaking_with_capacity(
            stream,
            crate::DEFAULT_BUFFER_SIZE,
            crate::ADMIN_MAX_BUFFER_SIZE,
        );
        trace!("accepted new session: {:?}", session.peer_addr());
        if self.poll.add_session(session).is_err() {
            increment_counter!(&Stat::TcpAcceptEx);
        }
    }

    /// Adds a new plain (non-TLS) session
    fn add_plain_session(&mut self, stream: TcpStream) {
        let session = Session::plain_with_capacity(
            stream,
            crate::DEFAULT_BUFFER_SIZE,
            crate::ADMIN_MAX_BUFFER_SIZE,
        );
        trace!("accepted new session: {:?}", session.peer_addr());
        if self.poll.add_session(session).is_err() {
            increment_counter!(&Stat::TcpAcceptEx);
        }
    }

    /// Repeatedly call accept on the listener
    fn do_accept(&mut self) {
        loop {
            match self.poll.accept() {
                Ok((stream, _)) => {
                    // handle TLS if it is configured
                    if let Some(ssl_context) = &self.ssl_context {
                        match Ssl::new(ssl_context).map(|v| v.accept(stream)) {
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

    fn handle_stats_request(session: &mut Session) {
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
            let _ = session.write(line.as_bytes());
        }
        let _ = session.write(b"END\r\n");
        increment_counter!(&Stat::AdminResponseCompose);
    }

    fn handle_version_request(session: &mut Session) {
        let _ = session.write(format!("VERSION {}\r\n", env!("CARGO_PKG_VERSION")).as_bytes());
        increment_counter!(&Stat::AdminResponseCompose);
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
        if let Ok(session) = self.poll.get_mut_session(token) {
            if session.is_handshaking() {
                if let Err(e) = session.do_handshake() {
                    if e.kind() == ErrorKind::WouldBlock {
                        // the session is still handshaking
                        return;
                    } else {
                        // some error occured while handshaking
                        let _ = self.poll.close_session(token);
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

        // run in a loop, accepting new sessions and events on existing sessions
        loop {
            increment_counter!(&Stat::AdminEventLoop);

            if self.poll.poll(&mut events, self.timeout).is_err() {
                error!("Error polling");
            }

            increment_counter_by!(
                &Stat::AdminEventTotal,
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
                        self.handle_session_event(event);
                    }
                }
            }

            self.get_rusage();

            self.log_receiver.flush();
        }
    }

    /// Returns a `SyncSender` which can be used to send `Message`s to the
    /// `Admin` component.
    pub fn signal_queue(&mut self) -> QueuePair<Signal, ()> {
        self.signal_queue.new_pair(128, None)
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
    fn handle_data(&mut self, token: Token) -> Result<(), std::io::Error> {
        trace!("handling request for admin session: {}", token.0);
        if let Ok(session) = self.poll.get_mut_session(token) {
            loop {
                if session.write_capacity() == 0 {
                    // if the write buffer is over-full, skip processing
                    break;
                }
                match self.parser.parse(session.buffer()) {
                    Ok(parsed_request) => {
                        let consumed = parsed_request.consumed();
                        let request = parsed_request.into_inner();
                        session.consume(consumed);

                        match request {
                            AdminRequest::Stats => {
                                Self::handle_stats_request(session);
                            }
                            AdminRequest::Quit => {
                                let _ = self.poll.close_session(token);
                                return Ok(());
                            }
                            AdminRequest::Version => {
                                Self::handle_version_request(session);
                            }
                        }
                    }
                    Err(ParseError::Incomplete) => {
                        break;
                    }
                    Err(_) => {
                        self.handle_error(token);
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::Other,
                            "bad request",
                        ));
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
        self.poll.reregister(token);
        Ok(())
    }

    fn poll(&mut self) -> &mut Poll {
        &mut self.poll
    }
}
