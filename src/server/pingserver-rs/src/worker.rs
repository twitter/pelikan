// Copyright 2020 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::common::Message;
use crate::event_loop::EventLoop;
use crate::session::*;
use crate::*;

use std::convert::TryInto;
use std::io::BufRead;
use std::sync::Arc;

/// A `Worker` handles events on `Session`s
pub struct Worker {
    config: Arc<PingserverConfig>,
    message_receiver: Receiver<Message>,
    message_sender: SyncSender<Message>,
    metrics: Arc<Metrics<AtomicU64, AtomicU64>>,
    poll: Poll,
    session_receiver: Receiver<Session>,
    session_sender: SyncSender<Session>,
    sessions: Slab<Session>,
}

impl Worker {
    /// Create a new `Worker` which will get new `Session`s from the MPSC queue
    pub fn new(
        config: Arc<PingserverConfig>,
        metrics: Arc<Metrics<AtomicU64, AtomicU64>>,
    ) -> Result<Self, std::io::Error> {
        let poll = Poll::new().map_err(|e| {
            error!("{}", e);
            std::io::Error::new(std::io::ErrorKind::Other, "Failed to create epoll instance")
        })?;
        let sessions = Slab::<Session>::new();

        let (session_sender, session_receiver) = sync_channel(128);
        let (message_sender, message_receiver) = sync_channel(128);

        Ok(Self {
            config,
            poll,
            message_receiver,
            message_sender,
            session_receiver,
            session_sender,
            sessions,
            metrics,
        })
    }

    /// Run the `Worker` in a loop, handling new session events
    pub fn run(&mut self) {
        let mut events = Events::with_capacity(self.config.worker().nevent());
        let timeout = Some(std::time::Duration::from_millis(
            self.config.worker().timeout() as u64,
        ));

        loop {
            let _ = self.metrics.increment_counter(&Stat::WorkerEventLoop, 1);

            // get events with timeout
            if self.poll.poll(&mut events, timeout).is_err() {
                error!("Error polling");
            }

            let _ = self.metrics.increment_counter(
                &Stat::WorkerEventTotal,
                events.iter().count().try_into().unwrap(),
            );

            // process all events
            for event in events.iter() {
                let token = event.token();

                // event for existing session
                trace!("got event for session: {}", token.0);

                // handle error events first
                if event.is_error() {
                    self.increment_count(&Stat::WorkerEventError);
                    self.handle_error(token);
                }

                // handle write events before read events to reduce write buffer
                // growth if there is also a readable event
                if event.is_writable() {
                    self.increment_count(&Stat::WorkerEventWrite);
                    self.do_write(token);
                }

                // read events are handled last
                if event.is_readable() {
                    self.increment_count(&Stat::WorkerEventRead);
                    let _ = self.do_read(token);
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
            }

            // poll queue to receive new sessions
            while let Ok(mut session) = self.session_receiver.try_recv() {
                let pending = session.buffer().read_pending();
                trace!("{} bytes pending in rx buffer for new session", pending);

                // reserve vacant slab
                let session_entry = self.sessions.vacant_entry();
                let token = Token(session_entry.key());

                // set client token to match slab
                session.set_token(token);

                // register tcp stream and insert into slab if successful
                match session.register(&self.poll) {
                    Ok(_) => {
                        session_entry.insert(session);
                        if pending > 0 {
                            // handle any pending data immediately
                            self.handle_data(token);
                        }
                    }
                    Err(_) => {
                        error!("Error registering new socket");
                    }
                };
            }

            // poll queue to receive new messages
            #[allow(clippy::never_loop)]
            while let Ok(message) = self.message_receiver.try_recv() {
                match message {
                    Message::Shutdown => {
                        return;
                    }
                }
            }
        }
    }

    pub fn message_sender(&self) -> SyncSender<Message> {
        self.message_sender.clone()
    }

    pub fn session_sender(&self) -> SyncSender<Session> {
        self.session_sender.clone()
    }
}

impl EventLoop for Worker {
    fn metrics(&self) -> &Arc<Metrics<AtomicU64, AtomicU64>> {
        &self.metrics
    }

    fn get_mut_session(&mut self, token: Token) -> Option<&mut Session> {
        self.sessions.get_mut(token.0)
    }

    fn handle_data(&mut self, token: Token) {
        trace!("handling request for session: {}", token.0);
        if let Some(session) = self.sessions.get_mut(token.0) {
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
                            let _ = self.metrics.increment_counter(&Stat::RequestParse, 1);
                            session.buffer().consume(6);
                            if session.write(b"PONG\r\n").is_err() {
                                // error writing
                                let _ = self.metrics.increment_counter(&Stat::ResponseComposeEx, 1);
                                self.handle_error(token);
                                return;
                            } else {
                                let _ = self.metrics.increment_counter(&Stat::ResponseCompose, 1);
                            }
                        } else {
                            // invalid command
                            debug!("error");
                            let _ = self.metrics.increment_counter(&Stat::RequestParseEx, 1);
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
            #[allow(clippy::collapsible_if)]
            if session.buffer().write_pending() > 0 {
                if session.flush().is_ok() && session.buffer().write_pending() > 0 {
                    self.reregister(token);
                }
            }
        } else {
            // no session for the token
            trace!(
                "attempted to handle data for non-existent session: {}",
                token.0
            );
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
