// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! The admin thread, which handles admin requests to return stats, get version
//! info, etc.

use crate::poll::{Poll, LISTENER_TOKEN, WAKER_TOKEN};
use crate::threads::EventLoop;
use crate::QUEUE_RETRIES;
use crate::TCP_ACCEPT_EX;
use crate::*;
use common::signal::Signal;
use common::ssl::{HandshakeError, MidHandshakeSslStream, Ssl, SslContext, SslStream};
use config::*;
use core::time::Duration;
use crossbeam_channel::Receiver;
use logger::Drain;
use metrics::{static_metrics, Counter, Gauge, Heatmap};
use mio::event::Event;
use mio::{Events, Token, Waker};
use protocol_admin::*;
use queues::Queues;
use session::{Session, TcpStream};
use std::io::{BufRead, Error, ErrorKind, Write};
use std::net::SocketAddr;
use std::sync::Arc;
use tiny_http::{Method, Request, Response};

static_metrics! {
    static ADMIN_REQUEST_PARSE: Counter;
    static ADMIN_RESPONSE_COMPOSE: Counter;
    static ADMIN_EVENT_ERROR: Counter;
    static ADMIN_EVENT_WRITE: Counter;
    static ADMIN_EVENT_READ: Counter;
    static ADMIN_EVENT_LOOP: Counter;
    static ADMIN_EVENT_TOTAL: Counter;

    static RU_UTIME: Counter;
    static RU_STIME: Counter;
    static RU_MAXRSS: Gauge;
    static RU_IXRSS: Gauge;
    static RU_IDRSS: Gauge;
    static RU_ISRSS: Gauge;
    static RU_MINFLT: Counter;
    static RU_MAJFLT: Counter;
    static RU_NSWAP: Counter;
    static RU_INBLOCK: Counter;
    static RU_OUBLOCK: Counter;
    static RU_MSGSND: Counter;
    static RU_MSGRCV: Counter;
    static RU_NSIGNALS: Counter;
    static RU_NVCSW: Counter;
    static RU_NIVCSW: Counter;
}

const KB: u64 = 1024; // one kilobyte in bytes
const S: u64 = 1_000_000_000; // one second in nanoseconds
const US: u64 = 1_000; // one microsecond in nanoseconds

pub static PERCENTILES: &[(&str, f64)] = &[
    ("p25", 25.0),
    ("p50", 50.0),
    ("p75", 75.0),
    ("p90", 90.0),
    ("p99", 99.0),
    ("p999", 99.9),
    ("p9999", 99.99),
];

pub struct AdminBuilder {
    addr: SocketAddr,
    nevent: usize,
    poll: Poll,
    timeout: Duration,
    ssl_context: Option<SslContext>,
    parser: AdminRequestParser,
    log_drain: Box<dyn Drain>,
    http_server: Option<tiny_http::Server>,
}

impl AdminBuilder {
    /// Creates a new `Admin` event loop.
    pub fn new<T: AdminConfig>(
        config: &T,
        ssl_context: Option<SslContext>,
        mut log_drain: Box<dyn Drain>,
    ) -> Result<Self, Error> {
        let config = config.admin();

        let addr = config.socket_addr().map_err(|e| {
            error!("{}", e);
            error!("bad admin listen address");
            let _ = log_drain.flush();
            Error::new(ErrorKind::Other, "bad listen address")
        })?;
        let mut poll = Poll::new().map_err(|e| {
            error!("{}", e);
            error!("failed to create epoll instance");
            let _ = log_drain.flush();
            Error::new(ErrorKind::Other, "failed to create epoll instance")
        })?;
        poll.bind(addr).map_err(|e| {
            error!("{}", e);
            error!("failed to bind admin tcp listener");
            let _ = log_drain.flush();
            Error::new(ErrorKind::Other, "failed to bind listener")
        })?;

        let ssl_context = if config.use_tls() { ssl_context } else { None };

        let timeout = std::time::Duration::from_millis(config.timeout() as u64);

        let nevent = config.nevent();

        let http_server = if config.http_enabled() {
            let addr = config.http_socket_addr().map_err(|e| {
                error!("{}", e);
                error!("bad admin http listen address");
                let _ = log_drain.flush();
                Error::new(ErrorKind::Other, "bad listen address")
            })?;
            let server = tiny_http::Server::http(addr).map_err(|e| {
                error!("{}", e);
                error!("could not start admin http server");
                let _ = log_drain.flush();
                Error::new(ErrorKind::Other, "failed to create http server")
            })?;
            Some(server)
        } else {
            None
        };

        Ok(Self {
            addr,
            timeout,
            nevent,
            poll,
            ssl_context,
            parser: AdminRequestParser::new(),
            log_drain,
            http_server,
        })
    }

    pub fn waker(&self) -> Arc<Waker> {
        self.poll.waker()
    }

    /// Triggers a flush of the log
    pub fn log_flush(&mut self) -> Result<(), std::io::Error> {
        self.log_drain.flush()
    }

    pub fn build(
        self,
        signal_queue_tx: Queues<Signal, ()>,
        signal_queue_rx: Receiver<Signal>,
    ) -> Admin {
        Admin {
            addr: self.addr,
            nevent: self.nevent,
            poll: self.poll,
            timeout: self.timeout,
            ssl_context: self.ssl_context,
            parser: self.parser,
            log_drain: self.log_drain,
            http_server: self.http_server,
            signal_queue_tx,
            signal_queue_rx,
        }
    }
}

pub struct Admin {
    addr: SocketAddr,
    nevent: usize,
    poll: Poll,
    timeout: Duration,
    ssl_context: Option<SslContext>,
    parser: AdminRequestParser,
    log_drain: Box<dyn Drain>,
    /// optional http server
    http_server: Option<tiny_http::Server>,
    /// used to send signals to all sibling threads
    signal_queue_tx: Queues<Signal, ()>,
    /// used to receive signals from the parent thread
    signal_queue_rx: Receiver<Signal>,
}

impl Drop for Admin {
    fn drop(&mut self) {
        let _ = self.log_drain.flush();
    }
}

impl Admin {
    /// Adds a new fully established TLS session
    fn add_established_tls_session(&mut self, stream: SslStream<TcpStream>) {
        let session = Session::tls_with_capacity(
            stream,
            crate::DEFAULT_BUFFER_SIZE,
            crate::ADMIN_MAX_BUFFER_SIZE,
        );
        if self.poll.add_session(session).is_err() {
            TCP_ACCEPT_EX.increment();
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
            TCP_ACCEPT_EX.increment();
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
            TCP_ACCEPT_EX.increment();
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
                                TCP_ACCEPT_EX.increment();
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

    /// This is a handler for the stats commands on the legacy admin port. It
    /// responses using the Memcached `stats` command response format, each stat
    /// appears on its own line with a CR+LF used as end of line symbol. The
    /// stats appear in sorted order.
    ///
    /// ```text
    /// STAT get 0
    /// STAT get_cardinality_p25 0
    /// STAT get_cardinality_p50 0
    /// STAT get_cardinality_p75 0
    /// STAT get_cardinality_p90 0
    /// STAT get_cardinality_p99 0
    /// STAT get_cardinality_p999 0
    /// STAT get_cardinality_p9999 0
    /// STAT get_ex 0
    /// STAT get_key 0
    /// STAT get_key_hit 0
    /// STAT get_key_miss 0
    /// ```
    fn handle_stats_request(session: &mut Session) {
        ADMIN_REQUEST_PARSE.increment();
        let mut data = Vec::new();
        for metric in &metrics::common::metrics::metrics() {
            let any = match metric.as_any() {
                Some(any) => any,
                None => {
                    continue;
                }
            };

            if let Some(counter) = any.downcast_ref::<Counter>() {
                data.push(format!("STAT {} {}\r\n", metric.name(), counter.value()));
            } else if let Some(gauge) = any.downcast_ref::<Gauge>() {
                data.push(format!("STAT {} {}\r\n", metric.name(), gauge.value()));
            } else if let Some(heatmap) = any.downcast_ref::<Heatmap>() {
                for (label, value) in PERCENTILES {
                    let percentile = heatmap.percentile(*value).unwrap_or(0);
                    data.push(format!(
                        "STAT {}_{} {}\r\n",
                        metric.name(),
                        label,
                        percentile
                    ));
                }
            }
        }

        data.sort();
        for line in data {
            let _ = session.write(line.as_bytes());
        }
        let _ = session.write(b"END\r\n");
        session.finalize_response();
        ADMIN_RESPONSE_COMPOSE.increment();
    }

    fn handle_version_request(session: &mut Session) {
        let _ = session.write(format!("VERSION {}\r\n", env!("CARGO_PKG_VERSION")).as_bytes());
        session.finalize_response();
        ADMIN_RESPONSE_COMPOSE.increment();
    }

    /// Handle an event on an existing session
    fn handle_session_event(&mut self, event: &Event) {
        let token = event.token();
        trace!("got event for admin session: {}", token.0);

        // handle error events first
        if event.is_error() {
            ADMIN_EVENT_ERROR.increment();
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
            ADMIN_EVENT_WRITE.increment();
            self.do_write(token);
        }

        // read events are handled last
        if event.is_readable() {
            ADMIN_EVENT_READ.increment();
            let _ = self.do_read(token);
        };
    }

    /// A "human-readable" exposition format which outputs one stat per line,
    /// with a LF used as the end of line symbol.
    ///
    /// ```text
    /// get: 0
    /// get_cardinality_p25: 0
    /// get_cardinality_p50: 0
    /// get_cardinality_p75: 0
    /// get_cardinality_p90: 0
    /// get_cardinality_p9999: 0
    /// get_cardinality_p999: 0
    /// get_cardinality_p99: 0
    /// get_ex: 0
    /// get_key: 0
    /// get_key_hit: 0
    /// get_key_miss: 0
    /// ```
    fn human_stats(&self) -> String {
        let mut data = Vec::new();

        for metric in &metrics::common::metrics::metrics() {
            let any = match metric.as_any() {
                Some(any) => any,
                None => {
                    continue;
                }
            };

            if let Some(counter) = any.downcast_ref::<Counter>() {
                data.push(format!("{}: {}", metric.name(), counter.value()));
            } else if let Some(gauge) = any.downcast_ref::<Gauge>() {
                data.push(format!("{}: {}", metric.name(), gauge.value()));
            } else if let Some(heatmap) = any.downcast_ref::<Heatmap>() {
                for (label, value) in PERCENTILES {
                    let percentile = heatmap.percentile(*value).unwrap_or(0);
                    data.push(format!("{}_{}: {}", metric.name(), label, percentile));
                }
            }
        }

        data.sort();
        data.join("\n") + "\n"
    }

    /// JSON stats output which follows the conventions found in Finagle and
    /// TwitterServer libraries. Percentiles are appended to the metric name,
    /// eg: `request_latency_p999` for the 99.9th percentile. For more details
    /// about the Finagle / TwitterServer format see:
    /// https://twitter.github.io/twitter-server/Features.html#metrics
    ///
    /// ```text
    /// {"get": 0,"get_cardinality_p25": 0,"get_cardinality_p50": 0, ... }
    /// ```
    fn json_stats(&self) -> String {
        let head = "{".to_owned();

        let mut data = Vec::new();

        for metric in &metrics::common::metrics::metrics() {
            let any = match metric.as_any() {
                Some(any) => any,
                None => {
                    continue;
                }
            };

            if let Some(counter) = any.downcast_ref::<Counter>() {
                data.push(format!("\"{}\": {}", metric.name(), counter.value()));
            } else if let Some(gauge) = any.downcast_ref::<Gauge>() {
                data.push(format!("\"{}\": {}", metric.name(), gauge.value()));
            } else if let Some(heatmap) = any.downcast_ref::<Heatmap>() {
                for (label, value) in PERCENTILES {
                    let percentile = heatmap.percentile(*value).unwrap_or(0);
                    data.push(format!("\"{}_{}\": {}", metric.name(), label, percentile));
                }
            }
        }

        data.sort();
        let body = data.join(",");
        let mut content = head;
        content += &body;
        content += "}";
        content
    }

    /// Prometheus / OpenTelemetry compatible stats output. Each stat is
    /// annotated with a type. Percentiles use the label 'percentile' to
    /// indicate which percentile corresponds to the value:
    ///
    /// ```text
    /// # TYPE get counter
    /// get 0
    /// # TYPE get_cardinality gauge
    /// get_cardinality{percentile="p25"} 0
    /// # TYPE get_cardinality gauge
    /// get_cardinality{percentile="p50"} 0
    /// # TYPE get_cardinality gauge
    /// get_cardinality{percentile="p75"} 0
    /// # TYPE get_cardinality gauge
    /// get_cardinality{percentile="p90"} 0
    /// # TYPE get_cardinality gauge
    /// get_cardinality{percentile="p99"} 0
    /// # TYPE get_cardinality gauge
    /// get_cardinality{percentile="p999"} 0
    /// # TYPE get_cardinality gauge
    /// get_cardinality{percentile="p9999"} 0
    /// # TYPE get_ex counter
    /// get_ex 0
    /// # TYPE get_key counter
    /// get_key 0
    /// # TYPE get_key_hit counter
    /// get_key_hit 0
    /// # TYPE get_key_miss counter
    /// get_key_miss 0
    /// ```
    fn prometheus_stats(&self) -> String {
        let mut data = Vec::new();

        for metric in &metrics::common::metrics::metrics() {
            let any = match metric.as_any() {
                Some(any) => any,
                None => {
                    continue;
                }
            };

            if let Some(counter) = any.downcast_ref::<Counter>() {
                data.push(format!(
                    "# TYPE {} counter\n{} {}",
                    metric.name(),
                    metric.name(),
                    counter.value()
                ));
            } else if let Some(gauge) = any.downcast_ref::<Gauge>() {
                data.push(format!(
                    "# TYPE {} gauge\n{} {}",
                    metric.name(),
                    metric.name(),
                    gauge.value()
                ));
            } else if let Some(heatmap) = any.downcast_ref::<Heatmap>() {
                for (label, value) in PERCENTILES {
                    let percentile = heatmap.percentile(*value).unwrap_or(0);
                    data.push(format!(
                        "# TYPE {} gauge\n{}{{percentile=\"{}\"}} {}",
                        metric.name(),
                        metric.name(),
                        label,
                        percentile
                    ));
                }
            }
        }
        data.sort();
        let mut content = data.join("\n");
        content += "\n";
        let parts: Vec<&str> = content.split('/').collect();
        parts.join("_")
    }

    /// Handle a HTTP request
    fn handle_http_request(&self, request: Request) {
        let url = request.url();
        let parts: Vec<&str> = url.split('?').collect();
        let url = parts[0];
        match url {
            // Prometheus/OpenTelemetry expect the `/metrics` URI will return
            // stats in the Prometheus format
            "/metrics" => match request.method() {
                Method::Get => {
                    let _ = request.respond(Response::from_string(self.prometheus_stats()));
                }
                _ => {
                    let _ = request.respond(Response::empty(400));
                }
            },
            // we export Finagle/TwitterServer format stats on a few endpoints
            // for maximum compatibility with various internal conventions
            "/metrics.json" | "/vars.json" | "/admin/metrics.json" => match request.method() {
                Method::Get => {
                    let _ = request.respond(Response::from_string(self.json_stats()));
                }
                _ => {
                    let _ = request.respond(Response::empty(400));
                }
            },
            // human-readable stats are exported on the `/vars` endpoint based
            // on internal conventions
            "/vars" => match request.method() {
                Method::Get => {
                    let _ = request.respond(Response::from_string(self.human_stats()));
                }
                _ => {
                    let _ = request.respond(Response::empty(400));
                }
            },
            _ => {
                let _ = request.respond(Response::empty(404));
            }
        }
    }

    /// Runs the `Admin` in a loop, accepting new sessions for the admin
    /// listener and handling events on existing sessions.
    pub fn run(&mut self) {
        info!("running admin on: {}", self.addr);

        let mut events = Events::with_capacity(self.nevent);

        // run in a loop, accepting new sessions and events on existing sessions
        loop {
            ADMIN_EVENT_LOOP.increment();

            if self.poll.poll(&mut events, self.timeout).is_err() {
                error!("Error polling");
            }

            ADMIN_EVENT_TOTAL.add(events.iter().count() as _);

            // handle all events
            for event in events.iter() {
                match event.token() {
                    LISTENER_TOKEN => {
                        self.do_accept();
                    }
                    WAKER_TOKEN => {
                        // check if we have received signals from any sibling
                        // thread
                        while let Ok(signal) = self.signal_queue_rx.try_recv() {
                            match signal {
                                Signal::FlushAll => {}
                                Signal::Shutdown => {
                                    // if a shutdown is received from any
                                    // thread, we will broadcast it to all
                                    // sibling threads and stop our event loop
                                    info!("shutting down");
                                    let _ = self.signal_queue_tx.try_send_all(Signal::Shutdown);
                                    if self.signal_queue_tx.wake().is_err() {
                                        fatal!("error waking threads for shutdown");
                                    }
                                    let _ = self.log_drain.flush();
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

            // handle all http requests if the http server is enabled
            if let Some(ref server) = self.http_server {
                while let Ok(Some(request)) = server.try_recv() {
                    self.handle_http_request(request);
                }
            }

            // handle all signals
            while let Ok(signal) = self.signal_queue_rx.try_recv() {
                match signal {
                    Signal::FlushAll => {}
                    Signal::Shutdown => {
                        // if a shutdown is received from any
                        // thread, we will broadcast it to all
                        // sibling threads and stop our event loop
                        info!("shutting down");
                        let _ = self.signal_queue_tx.try_send_all(Signal::Shutdown);
                        if self.signal_queue_tx.wake().is_err() {
                            fatal!("error waking threads for shutdown");
                        }
                        let _ = self.log_drain.flush();
                        return;
                    }
                }
            }

            // get updated usage
            self.get_rusage();

            // flush pending log entries to log destinations
            let _ = self.log_drain.flush();
        }
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
            RU_UTIME.set(rusage.ru_utime.tv_sec as u64 * S + rusage.ru_utime.tv_usec as u64 * US);
            RU_STIME.set(rusage.ru_stime.tv_sec as u64 * S + rusage.ru_stime.tv_usec as u64 * US);
            RU_MAXRSS.set(rusage.ru_maxrss * KB as i64);
            RU_IXRSS.set(rusage.ru_ixrss * KB as i64);
            RU_IDRSS.set(rusage.ru_idrss * KB as i64);
            RU_ISRSS.set(rusage.ru_isrss * KB as i64);
            RU_MINFLT.set(rusage.ru_minflt as u64);
            RU_MAJFLT.set(rusage.ru_majflt as u64);
            RU_NSWAP.set(rusage.ru_nswap as u64);
            RU_INBLOCK.set(rusage.ru_inblock as u64);
            RU_OUBLOCK.set(rusage.ru_oublock as u64);
            RU_MSGSND.set(rusage.ru_msgsnd as u64);
            RU_MSGRCV.set(rusage.ru_msgrcv as u64);
            RU_NSIGNALS.set(rusage.ru_nsignals as u64);
            RU_NVCSW.set(rusage.ru_nvcsw as u64);
            RU_NIVCSW.set(rusage.ru_nivcsw as u64);
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
                            AdminRequest::FlushAll => {
                                for _ in 0..QUEUE_RETRIES {
                                    if self.signal_queue_tx.try_send_all(Signal::FlushAll).is_ok() {
                                        warn!("sending flush_all signal");
                                        break;
                                    }
                                }
                                for _ in 0..QUEUE_RETRIES {
                                    if self.signal_queue_tx.wake().is_ok() {
                                        break;
                                    }
                                }

                                let _ = session.write(b"OK\r\n");
                                session.finalize_response();
                                ADMIN_RESPONSE_COMPOSE.increment();
                            }
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
