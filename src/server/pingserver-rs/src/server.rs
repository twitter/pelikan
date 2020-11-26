// Copyright 2020 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::event_loop::EventLoop;
use crate::session::*;
use crate::*;
use mio::net::TcpListener;

/// A `Server` is used to bind to a given socket address and accept new
/// sessions. These sessions are moved onto a MPSC queue, where they can be
/// handled by a `Worker`.
pub struct Server {
    addr: SocketAddr,
    config: Arc<PingserverConfig>,
    listener: TcpListener,
    poll: Poll,
    sender: SyncSender<Session>,
    waker: Arc<Waker>,
    tls_config: Option<Arc<rustls::ServerConfig>>,
    sessions: Slab<Session>,
}

pub const LISTENER_TOKEN: usize = usize::MAX;

impl Server {
    /// Creates a new `Server` that will bind to a given `addr` and push new
    /// `Session`s over the `sender`
    pub fn new(
        config: Arc<PingserverConfig>,
        sender: SyncSender<Session>,
        waker: Arc<Waker>,
    ) -> Result<Self, std::io::Error> {
        let addr = config.server().socket_addr().map_err(|e| {
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

        let tls_config = load_tls_config(&config)?;

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

        Ok(Self {
            addr,
            config,
            listener,
            poll,
            sender,
            waker,
            tls_config,
            sessions,
        })
    }

    /// Runs the `Server` in a loop, accepting new sessions and moving them to
    /// the queue
    pub fn run(&mut self) {
        info!("running server on: {}", self.addr);

        let mut events = Events::with_capacity(self.config.server().nevent());
        let timeout = Some(std::time::Duration::from_millis(
            self.config.server().timeout() as u64,
        ));

        // repeatedly run accepting new connections and moving them to the worker
        loop {
            if self.poll.poll(&mut events, timeout).is_err() {
                error!("Error polling server");
            }
            for event in events.iter() {
                if event.token() == Token(LISTENER_TOKEN) {
                    while let Ok((stream, addr)) = self.listener.accept() {
                        if let Some(tls_config) = &self.tls_config {
                            let mut session = Session::new(
                                addr,
                                stream,
                                State::Handshaking,
                                Some(rustls::ServerSession::new(&tls_config)),
                            );
                            let s = self.sessions.vacant_entry();
                            let token = s.key();
                            session.set_token(Token(token));
                            let _ = session.register(&self.poll);
                            s.insert(session);
                        } else {
                            let session = Session::new(addr, stream, State::Established, None);
                            trace!("accepted new session: {}", addr);
                            if self.sender.send(session).is_err() {
                                error!("error sending session to worker");
                            } else {
                                let _ = self.waker.wake();
                            }
                        };
                    }
                } else {
                    let token = event.token();
                    trace!("got event for session: {}", token.0);
                    let _read = if event.is_readable() {
                        self.do_read(token)
                    } else {
                        Ok(())
                    };

                    if event.is_writable() {
                        self.do_write(token);
                    }

                    if let Some(handshaking) =
                        self.sessions.get(token.0).map(|v| v.is_handshaking())
                    {
                        if !handshaking {
                            let mut session = self.sessions.remove(token.0);
                            let _ = session.deregister(&self.poll);
                            session.set_state(State::Established);
                            if self.sender.send(session).is_err() {
                                error!("error sending session to worker");
                            } else {
                                trace!("moving established session to worker");
                                let _ = self.waker.wake();
                            }
                        }
                    }
                }
            }
        }
    }
}

impl EventLoop for Server {
    fn get_mut_session<'a>(&'a mut self, token: Token) -> Option<&'a mut Session> {
        self.sessions.get_mut(token.0)
    }

    fn handle_data(&mut self, _token: Token) {}

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

fn load_tls_config(
    config: &Arc<PingserverConfig>,
) -> Result<Option<Arc<rustls::ServerConfig>>, std::io::Error> {
    let verifier = if let Some(certificate_chain) = config.tls().certificate_chain() {
        let mut certstore = rustls::RootCertStore::empty();
        let cafile = std::fs::File::open(certificate_chain).map_err(|e| {
            error!("{}", e);
            std::io::Error::new(std::io::ErrorKind::Other, "Could not open CA file")
        })?;
        certstore
            .add_pem_file(&mut std::io::BufReader::new(cafile))
            .map_err(|_| {
                std::io::Error::new(std::io::ErrorKind::Other, "Could not parse CA file")
            })?;
        Some(rustls::AllowAnyAnonymousOrAuthenticatedClient::new(
            certstore,
        ))
    } else {
        None
    };

    let cert = if let Some(certificate) = config.tls().certificate() {
        let certfile = std::fs::File::open(certificate).map_err(|e| {
            error!("{}", e);
            std::io::Error::new(std::io::ErrorKind::Other, "Could not open certificate file")
        })?;
        Some(
            rustls::internal::pemfile::certs(&mut std::io::BufReader::new(certfile)).map_err(
                |_| {
                    std::io::Error::new(
                        std::io::ErrorKind::Other,
                        "Could not parse certificate file",
                    )
                },
            )?,
        )
    } else {
        None
    };

    let key = if let Some(private_key) = config.tls().private_key() {
        let keyfile = std::fs::File::open(private_key).map_err(|e| {
            error!("{}", e);
            std::io::Error::new(std::io::ErrorKind::Other, "Could not open private key file")
        })?;
        let keys =
            rustls::internal::pemfile::pkcs8_private_keys(&mut std::io::BufReader::new(keyfile))
                .map_err(|_| {
                    std::io::Error::new(
                        std::io::ErrorKind::Other,
                        "Could not parse private key file",
                    )
                })?;
        if keys.len() != 1 {
            fatal!("Expected 1 private key, got: {}", keys.len());
        }
        Some(keys[0].clone())
    } else {
        None
    };

    if verifier.is_none() && cert.is_none() && key.is_none() {
        Ok(None)
    } else if verifier.is_some() && cert.is_some() && key.is_some() {
        let mut tls_config = rustls::ServerConfig::new(verifier.unwrap());
        let _ = tls_config.set_single_cert(cert.unwrap(), key.unwrap());
        Ok(Some(Arc::new(tls_config)))
    } else {
        error!("Incomplete TLS config");
        Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "Incomplete TLS config",
        ))
    }
}
