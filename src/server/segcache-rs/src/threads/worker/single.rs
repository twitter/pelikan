// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! The single-threaded worker, which is used when there is only one worker
//! thread configured. This worker parses buffers to produce requests, executes
//! the request using the backing storage, and then composes a response onto the
//! session buffer.

use crate::buffer::Buffer;
use crate::common::Queue;
use crate::common::Sender;
use crate::common::Signal;
use crate::event_loop::EventLoop;
use crate::session::*;
use crate::*;
use config::WorkerConfig;
use core::marker::PhantomData;
use metrics::Stat;
use mio::event::Event;
use mio::Events;
use mio::Poll;
use mio::Token;
use slab::Slab;
use std::convert::TryInto;
use std::sync::Arc;

/// A `Worker` handles events on `Session`s
pub struct SingleWorker<Storage, Request, Response> {
    storage: Storage,
    config: Arc<WorkerConfig>,
    poll: Poll,
    session_queue: Queue<Session>,
    sessions: Slab<Session>,
    signal_queue: Queue<Signal>,
    _request: PhantomData<Request>,
    _response: PhantomData<Response>,
}

impl<Storage, Request, Response> SingleWorker<Storage, Request, Response>
where
    Request: Parse<Buffer>,
    Response: Compose,
    Storage: Execute<Request, Response> + crate::storage::Storage,
{
    /// Create a new `Worker` which will get new `Session`s from the MPSC queue
    pub fn new(config: Arc<WorkerConfig>, storage: Storage) -> Result<Self, std::io::Error> {
        let poll = Poll::new().map_err(|e| {
            error!("{}", e);
            std::io::Error::new(std::io::ErrorKind::Other, "Failed to create epoll instance")
        })?;
        let sessions = Slab::<Session>::new();

        let session_queue = Queue::new(128);
        let signal_queue = Queue::new(128);

        Ok(Self {
            config,
            poll,
            storage,
            signal_queue,
            session_queue,
            sessions,
            _request: PhantomData,
            _response: PhantomData,
        })
    }

    /// Run the `Worker` in a loop, handling new session events
    pub fn run(&mut self) {
        let mut events = Events::with_capacity(self.config.nevent());
        let timeout = Some(std::time::Duration::from_millis(
            self.config.timeout() as u64
        ));

        #[cfg(feature = "heap_dump")]
        let mut ops = 0;
        #[cfg(feature = "heap_dump")]
        let mut seq = 0;

        loop {
            increment_counter!(&Stat::WorkerEventLoop);

            self.storage.expire();

            // get events with timeout
            if self.poll.poll(&mut events, timeout).is_err() {
                error!("Error polling");
            }

            increment_counter_by!(
                &Stat::WorkerEventTotal,
                events.iter().count().try_into().unwrap(),
            );

            // process all events
            for event in events.iter() {
                self.handle_event(event);
            }

            // poll queue to receive new sessions
            self.handle_new_sessions();

            // poll queue to receive new signals
            #[allow(clippy::never_loop)]
            while let Ok(signal) = self.signal_queue.try_recv() {
                match signal {
                    Signal::Shutdown => {
                        return;
                    }
                }
            }
        }
    }

    fn handle_new_sessions(&mut self) {
        while let Ok(mut session) = self.session_queue.try_recv() {
            let pending = session.read_pending();
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
                        if self.handle_data(token).is_err() {
                            self.handle_error(token);
                        }
                    }
                }
                Err(_) => {
                    error!("Error registering new socket");
                }
            };
        }
    }

    fn handle_event(&mut self, event: &Event) {
        let token = event.token();

        // event for existing session
        trace!("got event for session: {}", token.0);

        // handle error events first
        if event.is_error() {
            increment_counter!(&Stat::WorkerEventError);
            self.handle_error(token);
        }

        // handle write events before read events to reduce write buffer
        // growth if there is also a readable event
        if event.is_writable() {
            increment_counter!(&Stat::WorkerEventWrite);
            self.do_write(token);
        }

        // read events are handled last
        if event.is_readable() {
            increment_counter!(&Stat::WorkerEventRead);
            let _ = self.do_read(token);

            #[cfg(feature = "heap_dump")]
            {
                ops += 1;
                if ops >= 1_000_000 {
                    let dump = self.data.dump();
                    let serialized = serde_json::to_string(&dump).unwrap();
                    let mut file = std::fs::File::create(&format!("dump_{}.raw", seq)).unwrap();
                    let _ = file.write_all(serialized.as_bytes());
                    seq += 1;
                    ops = 0;
                }
            }
        }

        if let Some(session) = self.sessions.get_mut(token.0) {
            trace!(
                "{} bytes pending in rx buffer for session: {}",
                session.read_pending(),
                token.0
            );
            trace!(
                "{} bytes pending in tx buffer for session: {}",
                session.write_pending(),
                token.0
            )
        }
    }

    pub fn signal_sender(&self) -> Sender<Signal> {
        self.signal_queue.sender()
    }

    pub fn session_sender(&self) -> Sender<Session> {
        self.session_queue.sender()
    }
}

impl<Storage, Request, Response> EventLoop for SingleWorker<Storage, Request, Response>
where
    Request: Parse<Buffer>,
    Response: Compose,
    Storage: Execute<Request, Response> + crate::storage::Storage,
{
    fn get_mut_session(&mut self, token: Token) -> Option<&mut Session> {
        self.sessions.get_mut(token.0)
    }

    fn handle_data(&mut self, token: Token) -> Result<(), ()> {
        trace!("handling request for session: {}", token.0);
        if let Some(session) = self.sessions.get_mut(token.0) {
            loop {
                // TODO(bmartin): buffer should allow us to check remaining
                // write capacity.
                if session.write_pending() > MIN_BUFFER_SIZE {
                    // if the write buffer is over-full, skip processing
                    break;
                }
                match Parse::parse(&mut session.read_buffer) {
                    Ok(request) => {
                        increment_counter!(&Stat::ProcessReq);
                        let response = self.storage.execute(request);
                        response.compose(&mut session.write_buffer);
                    }
                    Err(ParseError::Incomplete) => {
                        break;
                    }
                    Err(_) => {
                        self.handle_error(token);
                        return Err(());
                    }
                }
            }
            #[allow(clippy::collapsible_if)]
            if session.write_pending() > 0 {
                if session.flush().is_ok() && session.write_pending() > 0 {
                    self.reregister(token);
                }
            }
            Ok(())
        } else {
            // no session for the token
            trace!(
                "attempted to handle data for non-existent session: {}",
                token.0
            );
            Ok(())
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
