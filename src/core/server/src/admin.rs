// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::*;
use protocol_common::BufMut;
use rustcommon_metrics::*;
use std::time::Duration;

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

pub struct Admin {
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
}

pub struct AdminBuilder {
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

        let waker = Arc::new(Waker::new(poll.registry(), WAKER_TOKEN).unwrap());

        let nevent = config.nevent();
        let timeout = Duration::from_millis(config.timeout() as u64);

        let sessions = Slab::new();

        let version = "unknown".to_string();

        Ok(Self {
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
            listener: self.listener,
            log_drain,
            nevent: self.nevent,
            poll: self.poll,
            sessions: self.sessions,
            signal_queue_rx,
            signal_queue_tx,
            timeout: self.timeout,
            version: self.version,
        }
    }
}

impl Admin {
    /// Call accept one time
    fn accept(&mut self) {
        if let Ok(mut session) = self
            .listener
            .accept()
            .map(|v| ServerSession::new(Session::from(v), AdminRequestParser::default()))
        {
            let s = self.sessions.vacant_entry();

            if session
                .register(self.poll.registry(), Token(s.key()), session.interest())
                .is_ok()
            {
                s.insert(session);
            } else {
                // failed to register
            }

            // reregister is needed here so we will call accept if there is a backlog
            if self
                .listener
                .reregister(self.poll.registry(), LISTENER_TOKEN, Interest::READABLE)
                .is_err()
            {
                // failed to reregister listener? how do we handle this?
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

                match session.flush() {
                    Ok(_) => Ok(()),
                    Err(e) => map_err(e),
                }?;

                if (session.write_pending() > 0 || session.remaining() > 0)
                    && session
                        .reregister(self.poll.registry(), token, session.interest())
                        .is_err()
                {
                    Err(Error::new(ErrorKind::Other, "failed to reregister"))
                } else {
                    Ok(())
                }
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
                    session.reregister(self.poll.registry(), token, session.interest())?;
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
            self.close(token);
            return;
        }

        if event.is_writable() && self.write(token).is_err() {
            self.close(token);
            return;
        }

        if event.is_readable() && self.read(token).is_err() {
            self.close(token);
            return;
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

    pub fn run(&mut self) {
        info!(
            "running admin on: {}",
            self.listener
                .local_addr()
                .map(|v| format!("{v}"))
                .unwrap_or_else(|_| "unknown address".to_string())
        );

        let mut events = Events::with_capacity(self.nevent);

        // repeatedly run accepting new connections and moving them to the worker
        loop {
            // ADMIN_EVENT_LOOP.increment();
            if self.poll.poll(&mut events, Some(self.timeout)).is_err() {
                error!("Error polling");
            }
            // ADMIN_EVENT_TOTAL.add(events.iter().count() as _);

            // handle all events
            for event in events.iter() {
                match event.token() {
                    LISTENER_TOKEN => {
                        self.accept();
                    }
                    WAKER_TOKEN => {
                        // do something here
                    }
                    _ => {
                        self.session_event(event);
                    }
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
fn handle_stats_request(session: &mut dyn BufMut) {
    // ADMIN_REQUEST_PARSE.increment();
    let mut data = Vec::new();
    for metric in &rustcommon_metrics::metrics() {
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
        session.put_slice(line.as_bytes());
    }
    session.put_slice(b"END\r\n");
    // ADMIN_RESPONSE_COMPOSE.increment();
}
