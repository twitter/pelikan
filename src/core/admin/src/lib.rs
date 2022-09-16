// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use ::net::event::{Event, Source};
use ::net::*;
use common::signal::Signal;
use common::ssl::tls_acceptor;
use config::{AdminConfig, TlsConfig};
use crossbeam_channel::Receiver;
use logger::*;
use protocol_admin::*;
use queues::Queues;
use rustcommon_metrics::*;
use session::{Buf, ServerSession, Session};
use slab::Slab;
use std::collections::VecDeque;
use std::io::{Error, ErrorKind, Result};
use std::sync::Arc;
use std::time::Duration;
use tiny_http::{Method, Request, Response};
use waker::Waker;

counter!(ADMIN_REQUEST_PARSE);
counter!(ADMIN_RESPONSE_COMPOSE);
counter!(ADMIN_EVENT_ERROR);
counter!(ADMIN_EVENT_WRITE);
counter!(ADMIN_EVENT_READ);
counter!(ADMIN_EVENT_LOOP);
counter!(ADMIN_EVENT_TOTAL);

counter!(RU_UTIME);
counter!(RU_STIME);
gauge!(RU_MAXRSS);
gauge!(RU_IXRSS);
gauge!(RU_IDRSS);
gauge!(RU_ISRSS);
counter!(RU_MINFLT);
counter!(RU_MAJFLT);
counter!(RU_NSWAP);
counter!(RU_INBLOCK);
counter!(RU_OUBLOCK);
counter!(RU_MSGSND);
counter!(RU_MSGRCV);
counter!(RU_NSIGNALS);
counter!(RU_NVCSW);
counter!(RU_NIVCSW);

counter!(
    ADMIN_SESSION_ACCEPT,
    "total number of attempts to accept a session"
);
counter!(
    ADMIN_SESSION_ACCEPT_EX,
    "number of times accept resulted in an exception, ignoring attempts that would block"
);
counter!(
    ADMIN_SESSION_ACCEPT_OK,
    "number of times a session was accepted successfully"
);

counter!(
    ADMIN_SESSION_CLOSE,
    "total number of times a session was closed"
);

gauge!(ADMIN_SESSION_CURR, "current number of admin sessions");

// consts

const LISTENER_TOKEN: Token = Token(usize::MAX - 1);
const WAKER_TOKEN: Token = Token(usize::MAX);

const KB: u64 = 1024; // one kilobyte in bytes
const S: u64 = 1_000_000_000; // one second in nanoseconds
const US: u64 = 1_000; // one microsecond in nanoseconds

// helper functions

fn map_err(e: std::io::Error) -> Result<()> {
    match e.kind() {
        ErrorKind::WouldBlock => Ok(()),
        _ => Err(e),
    }
}

pub struct Admin {
    /// A backlog of tokens that need to be handled
    backlog: VecDeque<Token>,
    http_server: Option<tiny_http::Server>,
    /// The actual network listener for the ASCII Admin Endpoint
    listener: ::net::Listener,
    /// The drain handle for the logger
    log_drain: Box<dyn Drain>,
    /// The maximum number of events to process per call to poll
    nevent: usize,
    /// The actual poll instantance
    poll: Poll,
    /// The sessions which have been opened
    sessions: Slab<ServerSession<AdminRequestParser, AdminResponse, AdminRequest>>,
    /// A queue for receiving signals from the parent thread
    signal_queue_rx: Receiver<Signal>,
    /// A set of queues for sending signals to sibling threads
    signal_queue_tx: Queues<Signal, ()>,
    /// The timeout for each call to poll
    timeout: Duration,
    /// The version of the service
    version: String,
    /// The waker for this thread
    waker: Arc<Waker>,
}

pub struct AdminBuilder {
    backlog: VecDeque<Token>,
    http_server: Option<tiny_http::Server>,
    listener: ::net::Listener,
    nevent: usize,
    poll: Poll,
    sessions: Slab<ServerSession<AdminRequestParser, AdminResponse, AdminRequest>>,
    timeout: Duration,
    version: String,
    waker: Arc<Waker>,
}

impl AdminBuilder {
    pub fn new<T: AdminConfig + TlsConfig>(config: &T) -> Result<Self> {
        let tls_config = config.tls();
        let config = config.admin();

        let addr = config.socket_addr().map_err(|e| {
            error!("{}", e);
            std::io::Error::new(std::io::ErrorKind::Other, "Bad listen address")
        })?;

        let tcp_listener = TcpListener::bind(addr)?;

        let mut listener = match (config.use_tls(), tls_acceptor(tls_config)?) {
            (true, Some(tls_acceptor)) => ::net::Listener::from((tcp_listener, tls_acceptor)),
            _ => ::net::Listener::from(tcp_listener),
        };

        let poll = Poll::new()?;
        listener.register(poll.registry(), LISTENER_TOKEN, Interest::READABLE)?;

        let waker = Arc::new(Waker::from(
            ::net::Waker::new(poll.registry(), WAKER_TOKEN).unwrap(),
        ));

        let nevent = config.nevent();
        let timeout = Duration::from_millis(config.timeout() as u64);

        let sessions = Slab::new();

        let version = "unknown".to_string();

        let backlog = VecDeque::new();

        let http_server = if config.http_enabled() {
            let addr = config.http_socket_addr().map_err(|_| {
                std::io::Error::new(std::io::ErrorKind::Other, "Bad HTTP listen address")
            })?;
            let server = tiny_http::Server::http(addr).map_err(|_| {
                std::io::Error::new(std::io::ErrorKind::Other, "Failed to create HTTP server")
            })?;
            Some(server)
        } else {
            None
        };

        Ok(Self {
            backlog,
            http_server,
            listener,
            nevent,
            poll,
            sessions,
            timeout,
            version,
            waker,
        })
    }

    pub fn version(&mut self, version: &str) {
        self.version = version.to_string();
    }

    pub fn waker(&self) -> Arc<Waker> {
        self.waker.clone()
    }

    pub fn build(
        self,
        log_drain: Box<dyn Drain>,
        signal_queue_rx: Receiver<Signal>,
        signal_queue_tx: Queues<Signal, ()>,
    ) -> Admin {
        Admin {
            backlog: self.backlog,
            http_server: self.http_server,
            listener: self.listener,
            log_drain,
            nevent: self.nevent,
            poll: self.poll,
            sessions: self.sessions,
            signal_queue_rx,
            signal_queue_tx,
            timeout: self.timeout,
            version: self.version,
            waker: self.waker,
        }
    }
}

fn get_rusage() {
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

impl Admin {
    /// Call accept one time
    fn accept(&mut self) {
        ADMIN_SESSION_ACCEPT.increment();

        match self
            .listener
            .accept()
            .map(|v| ServerSession::new(Session::from(v), AdminRequestParser::default()))
        {
            Ok(mut session) => {
                let s = self.sessions.vacant_entry();
                let interest = session.interest();
                if session
                    .register(self.poll.registry(), Token(s.key()), interest)
                    .is_ok()
                {
                    ADMIN_SESSION_ACCEPT_OK.increment();
                    ADMIN_SESSION_CURR.increment();

                    s.insert(session);
                } else {
                    // failed to register
                    ADMIN_SESSION_ACCEPT_EX.increment();
                }

                self.backlog.push_back(LISTENER_TOKEN);
                let _ = self.waker.wake();
            }
            Err(e) => {
                if e.kind() != ErrorKind::WouldBlock {
                    ADMIN_SESSION_ACCEPT_EX.increment();
                    self.backlog.push_back(LISTENER_TOKEN);
                    let _ = self.waker.wake();
                }
            }
        }
    }

    fn read(&mut self, token: Token) -> Result<()> {
        let session = self
            .sessions
            .get_mut(token.0)
            .ok_or_else(|| Error::new(ErrorKind::Other, "non-existant session"))?;

        // fill the session
        match session.fill() {
            Ok(0) => Err(Error::new(ErrorKind::Other, "client hangup")),
            r => r,
        }?;

        match session.receive() {
            Ok(request) => {
                ADMIN_REQUEST_PARSE.increment();

                // do some request handling
                match request {
                    AdminRequest::FlushAll => {
                        let _ = self.signal_queue_tx.try_send_all(Signal::FlushAll);
                        session.send(AdminResponse::Ok)?;
                    }
                    AdminRequest::Quit => {
                        return Err(Error::new(ErrorKind::Other, "should hangup"));
                    }
                    AdminRequest::Stats => {
                        session.send(AdminResponse::Stats)?;
                    }
                    AdminRequest::Version => {
                        session.send(AdminResponse::version(self.version.clone()))?;
                    }
                }

                ADMIN_RESPONSE_COMPOSE.increment();

                match session.flush() {
                    Ok(_) => Ok(()),
                    Err(e) => map_err(e),
                }?;

                if session.write_pending() > 0 || session.remaining() > 0 {
                    let interest = session.interest();
                    if session
                        .reregister(self.poll.registry(), token, interest)
                        .is_err()
                    {
                        return Err(Error::new(ErrorKind::Other, "failed to reregister"));
                    }
                }
                Ok(())
            }
            Err(e) => match e.kind() {
                ErrorKind::WouldBlock => Ok(()),
                _ => Err(e),
            },
        }
    }

    fn write(&mut self, token: Token) -> Result<()> {
        let session = self
            .sessions
            .get_mut(token.0)
            .ok_or_else(|| Error::new(ErrorKind::Other, "non-existant session"))?;

        match session.flush() {
            Ok(_) => Ok(()),
            Err(e) => match e.kind() {
                ErrorKind::WouldBlock => Ok(()),
                _ => Err(e),
            },
        }
    }

    /// Closes the session with the given token
    fn close(&mut self, token: Token) {
        if self.sessions.contains(token.0) {
            ADMIN_SESSION_CLOSE.increment();
            ADMIN_SESSION_CURR.decrement();

            let mut session = self.sessions.remove(token.0);
            let _ = session.flush();
        }
    }

    fn handshake(&mut self, token: Token) -> Result<()> {
        let session = self
            .sessions
            .get_mut(token.0)
            .ok_or_else(|| Error::new(ErrorKind::Other, "non-existant session"))?;

        match session.do_handshake() {
            Ok(()) => {
                if session.remaining() > 0 {
                    let interest = session.interest();
                    session.reregister(self.poll.registry(), token, interest)?;
                    Ok(())
                } else {
                    Ok(())
                }
            }
            Err(e) => Err(e),
        }
    }

    /// handle a single session event
    fn session_event(&mut self, event: &Event) {
        let token = event.token();

        if event.is_error() {
            ADMIN_EVENT_ERROR.increment();

            self.close(token);
            return;
        }

        if event.is_writable() {
            ADMIN_EVENT_WRITE.increment();

            if self.write(token).is_err() {
                self.close(token);
                return;
            }
        }

        if event.is_readable() {
            ADMIN_EVENT_READ.increment();

            if self.read(token).is_err() {
                self.close(token);
                return;
            }
        }

        match self.handshake(token) {
            Ok(_) => {}
            Err(e) => match e.kind() {
                ErrorKind::WouldBlock => {}
                _ => {
                    self.close(token);
                }
            },
        }
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

        for metric in &rustcommon_metrics::metrics() {
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

        for metric in &rustcommon_metrics::metrics() {
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

        for metric in &rustcommon_metrics::metrics() {
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

    pub fn run(&mut self) {
        info!(
            "running admin on: {}",
            self.listener
                .local_addr()
                .map(|v| format!("{v}"))
                .unwrap_or_else(|_| "unknown address".to_string())
        );

        let mut events = Events::with_capacity(self.nevent);

        loop {
            ADMIN_EVENT_LOOP.increment();

            get_rusage();

            if self.poll.poll(&mut events, Some(self.timeout)).is_err() {
                error!("Error polling");
            }

            ADMIN_EVENT_TOTAL.add(events.iter().count() as _);

            // handle all events
            for event in events.iter() {
                match event.token() {
                    LISTENER_TOKEN => {
                        self.accept();
                    }
                    WAKER_TOKEN => {
                        self.waker.reset();
                        let tokens: Vec<Token> = self.backlog.drain(..).collect();
                        for token in tokens {
                            if token == LISTENER_TOKEN {
                                self.accept();
                            }
                        }
                    }
                    _ => {
                        self.session_event(event);
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

            // flush pending log entries to log destinations
            let _ = self.log_drain.flush();
        }
    }
}

common::metrics::test_no_duplicates!();
