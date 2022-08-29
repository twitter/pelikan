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
use session::{ServerSession, Session};
use slab::Slab;
use std::collections::VecDeque;
use std::io::{Error, ErrorKind, Result};
use std::sync::Arc;
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

// new stats below

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
    waker: Arc<Box<dyn waker::Waker>>,
}

pub struct AdminBuilder {
    backlog: VecDeque<Token>,
    listener: ::net::Listener,
    nevent: usize,
    poll: Poll,
    sessions: Slab<ServerSession<AdminRequestParser, AdminResponse, AdminRequest>>,
    timeout: Duration,
    version: String,
    waker: Arc<Box<dyn waker::Waker>>,
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

        let waker =
            Arc::new(Box::new(Waker::new(poll.registry(), WAKER_TOKEN).unwrap())
                as Box<dyn waker::Waker>);

        let nevent = config.nevent();
        let timeout = Duration::from_millis(config.timeout() as u64);

        let sessions = Slab::new();

        let version = "unknown".to_string();

        let backlog = VecDeque::new();

        Ok(Self {
            backlog,
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

    pub fn waker(&self) -> Arc<Box<dyn waker::Waker>> {
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

                if session
                    .register(self.poll.registry(), Token(s.key()), session.interest())
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
        ADMIN_EVENT_READ.increment();

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
        ADMIN_EVENT_WRITE.increment();

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
            ADMIN_EVENT_LOOP.increment();

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
