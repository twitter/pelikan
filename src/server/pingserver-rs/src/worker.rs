// Copyright 2020 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::event_loop::EventLoop;
use crate::session::*;
use crate::*;

use std::io::BufRead;
use std::sync::Arc;

/// A `Worker` handles events on `Session`s
pub struct Worker {
    config: Arc<PingserverConfig>,
    sessions: Slab<Session>,
    poll: Poll,
    receiver: Receiver<Session>,
    waker: Arc<Waker>,
    waker_token: Token,
}

pub const WAKER_TOKEN: usize = usize::MAX;

impl Worker {
    /// Create a new `Worker` which will get new `Session`s from the MPSC queue
    pub fn new(
        config: Arc<PingserverConfig>,
        receiver: Receiver<Session>,
    ) -> Result<Self, std::io::Error> {
        let poll = Poll::new().map_err(|e| {
            error!("{}", e);
            std::io::Error::new(std::io::ErrorKind::Other, "Failed to create epoll instance")
        })?;
        let sessions = Slab::<Session>::new();
        let waker_token = Token(WAKER_TOKEN);
        let waker = Arc::new(Waker::new(&poll.registry(), waker_token)?);

        Ok(Self {
            config,
            poll,
            receiver,
            sessions,
            waker,
            waker_token,
        })
    }

    /// Run the `Worker` in a loop, handling new session events
    pub fn run(&mut self) -> Self {
        let mut events = Events::with_capacity(self.config.worker().nevent());
        let timeout = Some(std::time::Duration::from_millis(
            self.config.worker().timeout() as u64,
        ));

        loop {
            // get client events with timeout
            if self.poll.poll(&mut events, timeout).is_err() {
                error!("Error polling");
            }

            // process all events
            for event in events.iter() {
                let token = event.token();

                if token != self.waker_token {
                    trace!("got event for session: {}", token.0);

                    if event.is_readable() {
                        let _ = self.do_read(token);
                    }

                    if event.is_writable() {
                        self.do_write(token);
                    }

                    if let Some(session) = self.sessions.get_mut(token.0) {
                        trace!(
                            "{} bytes pending in rx buffer for session: {}",
                            session.buffer().read_pending(),
                            token.0
                        );
                        trace!(
                            "{} bytes pending in tx buffer for session: {}",
                            session.buffer().write_pending(),
                            token.0
                        )
                    }
                } else {
                    // handle new connections
                    while let Ok(mut s) = self
                        .receiver
                        .recv_timeout(std::time::Duration::from_millis(1))
                    {
                        let pending = s.buffer().read_pending();
                        trace!("{} bytes pending in rx buffer for new session", pending);

                        // reserve vacant slab
                        let session = self.sessions.vacant_entry();
                        let token = Token(session.key());

                        // set client token to match slab
                        s.set_token(token);

                        // register tcp stream and insert into slab if successful
                        match s.register(&self.poll) {
                            Ok(_) => {
                                session.insert(s);
                                if pending > 0 {
                                    self.handle_data(token);
                                }
                            }
                            Err(_) => {
                                error!("Error registering new socket");
                            }
                        };
                    }
                }
            }
        }
    }

    pub fn waker(&self) -> Arc<Waker> {
        self.waker.clone()
    }
}

impl EventLoop for Worker {
    fn get_mut_session<'a>(&'a mut self, token: Token) -> Option<&'a mut Session> {
        self.sessions.get_mut(token.0)
    }

    fn handle_data(&mut self, token: Token) {
        trace!("handling request for session: {}", token.0);
        if let Some(session) = self.get_mut_session(token) {
            loop {
                // TODO(bmartin): buffer should allow us to check remaining
                // write capacity.
                if session.buffer().write_pending() > (1024 - 6) {
                    // if the write buffer is over-full, skip processing
                    break;
                }
                match session.buffer().fill_buf() {
                    Ok(buf) => {
                        if buf.len() < 6 {
                            // Shortest request is "PING\r\n" at 6 bytes
                            // All complete responses end in CRLF

                            // incomplete request, stay in reading
                            break;
                        } else if &buf[0..6] == b"PING\r\n" {
                            session.buffer().consume(6);
                            if session.write(b"PONG\r\n").is_err() {
                                // error writing
                                self.handle_error(token);
                                return;
                            }
                        } else {
                            // invalid command
                            debug!("error");
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

    fn take_session(&mut self, token: Token) -> Option<Session> {
        if self.sessions.contains(token.0) {
            let session = self.sessions.remove(token.0);
            Some(session)
        } else {
            None
        }
    }

    fn poll(&self) -> &Poll {
        &self.poll
    }
}
