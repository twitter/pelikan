use crate::*;

pub struct Admin {
    listener: ::net::Listener,
    log_drain: Box<dyn Drain>,
    poll: Poll,
    sessions: Slab<ServerSession<AdminRequestParser, AdminResponse, AdminRequest>>,
    signal_queue_rx: Receiver<Signal>,
    signal_queue_tx: Queues<Signal, ()>,
    timeout: Duration,
    waker: Arc<Waker>,
}

pub struct AdminBuilder {
    listener: ::net::Listener,
    poll: Poll,
    sessions: Slab<ServerSession<AdminRequestParser, AdminResponse, AdminRequest>>,
    timeout: Duration,
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

        // let nevent = config.nevent();
        let timeout = Duration::from_millis(config.timeout() as u64);

        let sessions = Slab::new();

        Ok(Self {
            listener,
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
        log_drain: Box<dyn Drain>,
        signal_queue_rx: Receiver<Signal>,
        signal_queue_tx: Queues<Signal, ()>,
    ) -> Admin {
        Admin {
            listener: self.listener,
            log_drain,
            poll: self.poll,
            sessions: self.sessions,
            signal_queue_rx,
            signal_queue_tx,
            timeout: self.timeout,
            waker: self.waker,
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

        match session.receive() {
            Ok(_request) => {
                // do some request handling

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
        // info!("running admin on: {}", self.addr);

        let mut events = Events::with_capacity(1024);

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
        }
    }
}
