use crate::*;
use rustcommon_metrics::*;
use std::time::Duration;

counter!(LISTENER_EVENT_ERROR);
counter!(LISTENER_EVENT_WRITE);
counter!(LISTENER_EVENT_READ);
counter!(LISTENER_EVENT_LOOP);
counter!(LISTENER_EVENT_TOTAL);

counter!(LISTENER_SESSION_DISCARD);

pub struct Listener {
    /// The actual network listener server
    listener: ::net::Listener,
    /// The maximum number of events to process per call to poll
    nevent: usize,
    /// The actual poll instantance
    poll: Poll,
    /// Sessions which have been opened, but are not fully established
    sessions: Slab<Session>,
    /// Queues for sending established sessions to the worker thread(s) and to
    /// receive sessions which should be closed
    session_queue: Queues<Session, Session>,
    /// Queue for receieving signals from the admin thread
    signal_queue: Queues<(), Signal>,
    /// The timeout for each call to poll
    timeout: Duration,
    /// The waker handle for this thread
    waker: Arc<Waker>,
}

pub struct ListenerBuilder {
    listener: ::net::Listener,
    nevent: usize,
    poll: Poll,
    sessions: Slab<Session>,
    timeout: Duration,
    waker: Arc<Waker>,
}

impl ListenerBuilder {
    pub fn new<T: ServerConfig + TlsConfig>(config: &T) -> Result<Self> {
        let tls_config = config.tls();
        let config = config.server();

        let addr = config.socket_addr().map_err(|e| {
            error!("{}", e);
            std::io::Error::new(std::io::ErrorKind::Other, "Bad listen address")
        })?;

        let tcp_listener = TcpListener::bind(addr)?;

        let mut listener = if let Some(tls_acceptor) = tls_acceptor(tls_config)? {
            ::net::Listener::from((tcp_listener, tls_acceptor))
        } else {
            ::net::Listener::from(tcp_listener)
        };

        let poll = Poll::new()?;
        listener.register(poll.registry(), LISTENER_TOKEN, Interest::READABLE)?;

        let waker = Arc::new(Waker::new(poll.registry(), WAKER_TOKEN).unwrap());

        let nevent = config.nevent();
        let timeout = Duration::from_millis(config.timeout() as u64);

        let sessions = Slab::new();

        Ok(Self {
            listener,
            nevent,
            poll,
            sessions,
            timeout,
            waker,
        })
    }

    pub fn waker(&self) -> Arc<Waker> {
        self.waker.clone()
    }

    pub fn build(
        self,
        signal_queue: Queues<(), Signal>,
        session_queue: Queues<Session, Session>,
    ) -> Listener {
        Listener {
            listener: self.listener,
            nevent: self.nevent,
            poll: self.poll,
            sessions: self.sessions,
            session_queue,
            signal_queue,
            timeout: self.timeout,
            waker: self.waker,
        }
    }
}

impl Listener {
    /// Call accept one time
    fn accept(&mut self) {
        if let Ok(mut session) = self.listener.accept().map(Session::from) {
            if session.is_handshaking() {
                let s = self.sessions.vacant_entry();
                if session
                    .register(self.poll.registry(), Token(s.key()), session.interest())
                    .is_ok()
                {
                    s.insert(session);
                } else {
                    // failed to register
                }
            } else if self.session_queue.try_send_any(session).is_err() {
                // we probably want to wake and retry here
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

    /// Handle a read event for the `Session` with the `Token`. This primarily
    /// just checks that there wasn't a hangup, as indicated by a zero-sized
    /// return from `read()`.
    fn read(&mut self, token: Token) -> Result<()> {
        let session = self
            .sessions
            .get_mut(token.0)
            .ok_or_else(|| Error::new(ErrorKind::Other, "non-existant session"))?;

        // read from session to buffer
        match session.fill() {
            Ok(0) => {
                // zero-length reads indicate remote side has closed connection
                trace!("hangup for session: {:?}", session);
                Err(Error::new(ErrorKind::Other, "client hangup"))
            }
            Ok(bytes) => {
                trace!("read {} bytes for session: {:?}", bytes, session);
                Ok(())
            }
            Err(e) => {
                match e.kind() {
                    ErrorKind::WouldBlock => {
                        // spurious read, ignore
                        Ok(())
                    }
                    _ => Err(e),
                }
            }
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

        session.do_handshake()
    }

    /// handle a single session event
    fn session_event(&mut self, event: &Event) {
        let token = event.token();

        if event.is_error() {
            LISTENER_EVENT_ERROR.increment();
            self.close(token);
            return;
        }

        if event.is_readable() {
            LISTENER_EVENT_READ.increment();
            if self.read(token).is_err() {
                self.close(token);
                return;
            }
        }

        match self.handshake(token) {
            Ok(_) => {
                // handshake is complete, send the session to a worker thread
                let mut session = self.sessions.remove(token.0);
                for attempt in 1..=QUEUE_RETRIES {
                    if let Err(s) = self.session_queue.try_send_any(session) {
                        if attempt == QUEUE_RETRIES {
                            LISTENER_SESSION_DISCARD.increment();
                        } else {
                            let _ = self.session_queue.wake();
                        }
                        session = s;
                    } else {
                        break;
                    }
                }
            }
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
            "running server on: {}",
            self.listener
                .local_addr()
                .map(|v| format!("{v}"))
                .unwrap_or_else(|_| "unknown address".to_string())
        );

        let mut events = Events::with_capacity(self.nevent);

        // repeatedly run accepting new connections and moving them to the worker
        loop {
            LISTENER_EVENT_LOOP.increment();
            if self.poll.poll(&mut events, Some(self.timeout)).is_err() {
                error!("Error polling server");
            }
            LISTENER_EVENT_TOTAL.add(events.iter().count() as _);

            // handle all events
            for event in events.iter() {
                match event.token() {
                    LISTENER_TOKEN => {
                        self.accept();
                    }
                    WAKER_TOKEN => {
                        // handle any closing sessions
                        if let Some(mut session) =
                            self.session_queue.try_recv().map(|v| v.into_inner())
                        {
                            let _ = session.flush();

                            // wakeup to handle the possibility of more sessions
                            let _ = self.waker.wake();
                        }

                        // check if we received any signals from the admin thread
                        while let Some(signal) =
                            self.signal_queue.try_recv().map(|v| v.into_inner())
                        {
                            match signal {
                                Signal::FlushAll => {}
                                Signal::Shutdown => {
                                    // if we received a shutdown, we can return
                                    // and stop processing events
                                    return;
                                }
                            }
                        }
                    }
                    _ => {
                        self.session_event(event);
                    }
                }
            }

            let _ = self.session_queue.wake();
        }
    }
}
