// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! The single-threaded worker, which is used when there is only one worker
//! thread configured. This worker parses buffers to produce requests, executes
//! the request using the backing storage, and then composes a response onto the
//! session buffer.

use super::EventLoop;
use super::*;
use crate::poll::{Poll, WAKER_TOKEN};
use common::signal::Signal;
use config::WorkerConfig;
use core::marker::PhantomData;
use core::time::Duration;
use entrystore::EntryStore;
use mio::event::Event;
use mio::Events;
use mio::Token;
use mio::Waker;
use protocol::{Compose, Execute, Parse, ParseError};
use queues::QueuePair;
use queues::QueuePairs;
use session::Session;
use std::io::{BufRead, Write};
use std::sync::Arc;

/// A `Worker` handles events on `Session`s
pub struct SingleWorker<Storage, Parser, Request, Response> {
    storage: Storage,
    poll: Poll,
    nevent: usize,
    timeout: Duration,
    session_queue: QueuePairs<(), Session>,
    signal_queue: QueuePairs<(), Signal>,
    _request: PhantomData<Request>,
    _response: PhantomData<Response>,
    parser: Parser,
}

impl<Storage, Parser, Request, Response> SingleWorker<Storage, Parser, Request, Response>
where
    Parser: Parse<Request>,
    Response: Compose,
    Storage: Execute<Request, Response> + EntryStore,
{
    /// Create a new `Worker` which will get new `Session`s from the MPSC queue
    pub fn new(
        config: &WorkerConfig,
        storage: Storage,
        parser: Parser,
    ) -> Result<Self, std::io::Error> {
        let poll = Poll::new().map_err(|e| {
            error!("{}", e);
            std::io::Error::new(std::io::ErrorKind::Other, "Failed to create epoll instance")
        })?;

        let session_queue = QueuePairs::new(Some(poll.waker()));
        let signal_queue = QueuePairs::new(Some(poll.waker()));

        Ok(Self {
            poll,
            nevent: config.nevent(),
            timeout: Duration::from_millis(config.timeout() as u64),
            storage,
            signal_queue,
            session_queue,
            _request: PhantomData,
            _response: PhantomData,
            parser,
        })
    }

    /// Run the `Worker` in a loop, handling new session events
    pub fn run(&mut self) {
        let mut events = Events::with_capacity(self.nevent);

        loop {
            WORKER_EVENT_LOOP.increment();

            self.storage.expire();

            // get events with timeout
            if self.poll.poll(&mut events, self.timeout).is_err() {
                error!("Error polling");
            }

            WORKER_EVENT_TOTAL.add(events.iter().count() as _);

            // process all events
            for event in events.iter() {
                match event.token() {
                    WAKER_TOKEN => {
                        self.handle_new_sessions();

                        #[allow(clippy::never_loop)]
                        while let Ok(signal) = self.signal_queue.recv_from(0) {
                            match signal {
                                Signal::Shutdown => {
                                    return;
                                }
                            }
                        }
                    }
                    _ => {
                        self.handle_event(&event);
                    }
                }
            }
        }
    }

    fn handle_new_sessions(&mut self) {
        while let Ok(session) = self.session_queue.recv_from(0) {
            let pending = session.read_pending();
            trace!("{} bytes pending in rx buffer for new session", pending);

            // reserve vacant slab
            if let Ok(token) = self.poll.add_session(session) {
                if pending > 0 {
                    // handle any pending data immediately
                    if self.handle_data(token).is_err() {
                        self.handle_error(token);
                    }
                }
            }
        }
    }

    fn handle_event(&mut self, event: &Event) {
        let token = event.token();

        // event for existing session
        trace!("got event for session: {}", token.0);

        // handle error events first
        if event.is_error() {
            WORKER_EVENT_ERROR.increment();
            self.handle_error(token);
        }

        // handle write events before read events to reduce write buffer
        // growth if there is also a readable event
        if event.is_writable() {
            WORKER_EVENT_WRITE.increment();
            self.do_write(token);
        }

        // read events are handled last
        if event.is_readable() {
            WORKER_EVENT_READ.increment();
            let _ = self.do_read(token);
        }

        if let Ok(session) = self.poll.get_mut_session(token) {
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

    pub fn session_sender(&mut self, waker: Arc<Waker>) -> QueuePair<Session, ()> {
        self.session_queue.new_pair(65536, Some(waker))
    }

    pub fn signal_queue(&mut self) -> QueuePair<Signal, ()> {
        self.signal_queue.new_pair(128, None)
    }
}

impl<Storage, Parser, Request, Response> EventLoop
    for SingleWorker<Storage, Parser, Request, Response>
where
    Parser: Parse<Request>,
    Response: Compose,
    Storage: Execute<Request, Response> + EntryStore,
{
    fn handle_data(&mut self, token: Token) -> Result<(), std::io::Error> {
        trace!("handling request for session: {}", token.0);
        if let Ok(session) = self.poll.get_mut_session(token) {
            loop {
                if session.write_capacity() == 0 {
                    // if the write buffer is over-full, skip processing
                    break;
                }
                match self.parser.parse(session.buffer()) {
                    Ok(parsed_request) => {
                        PROCESS_REQ.increment();
                        let consumed = parsed_request.consumed();
                        let request = parsed_request.into_inner();
                        session.consume(consumed);

                        if let Some(response) = self.storage.execute(request) {
                            response.compose(session);
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
            #[allow(clippy::collapsible_if)]
            if session.write_pending() > 0 {
                if session.flush().is_ok() && session.write_pending() > 0 {
                    self.poll.reregister(token);
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

    fn poll(&mut self) -> &mut Poll {
        &mut self.poll
    }
}
