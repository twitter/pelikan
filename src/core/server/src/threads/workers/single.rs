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
use protocol_common::{Compose, Execute, ParseError};
use session::Session;
use std::io::{BufRead, Write};
use std::sync::Arc;

/// A builder type for a single-threaded worker which owns the storage.
pub struct SingleWorkerBuilder<Storage, Server, Request, Response> {
    nevent: usize,
    server: Server,
    poll: Poll,
    timeout: Duration,
    storage: Storage,
    _request: PhantomData<Request>,
    _response: PhantomData<Response>,
}

impl<Storage, Server, Request, Response> SingleWorkerBuilder<Storage, Server, Request, Response>
where
    Server: service_common::Server<Request, Response>,
{
    /// Create a new builder for a single-threaded worker from the provided
    /// config, storage, and parser
    pub fn new<T: WorkerConfig>(
        config: &T,
        storage: Storage,
        server: Server,
    ) -> Result<Self, std::io::Error> {
        let poll = Poll::new().map_err(|e| {
            error!("{}", e);
            std::io::Error::new(std::io::ErrorKind::Other, "Failed to create epoll instance")
        })?;

        Ok(Self {
            poll,
            nevent: config.worker().nevent(),
            timeout: Duration::from_millis(config.worker().timeout() as u64),
            storage,
            _request: PhantomData,
            _response: PhantomData,
            server,
        })
    }

    /// Returns the waker for this worker.
    pub(crate) fn waker(&self) -> Arc<Waker> {
        self.poll.waker()
    }

    /// Finalize the builder and return a `SingleWorker` by providing the queues
    /// that are required to communicate with other threads.
    pub fn build(
        self,
        signal_queue: Queues<(), Signal>,
        session_queue: Queues<(), Session>,
    ) -> SingleWorker<Storage, Server, Request, Response> {
        SingleWorker {
            nevent: self.nevent,
            server: self.server,
            poll: self.poll,
            timeout: self.timeout,
            storage: self.storage,
            session_queue,
            signal_queue,
            _request: PhantomData,
            _response: PhantomData,
        }
    }
}

/// A finalized single-threaded worker which is ready to be run.
pub struct SingleWorker<Storage, Server, Request, Response> {
    nevent: usize,
    server: Server,
    poll: Poll,
    timeout: Duration,
    storage: Storage,
    session_queue: Queues<(), Session>,
    signal_queue: Queues<(), Signal>,
    _request: PhantomData<Request>,
    _response: PhantomData<Response>,
}

impl<Storage, Server, Request, Response> SingleWorker<Storage, Server, Request, Response>
where
    Server: service_common::Server<Request, Response>,
    Response: Compose,
    Storage: Execute<Request, Response> + EntryStore,
{
    /// Run the worker in a loop, handling new events.
    pub fn run(&mut self) {
        let mut events = Events::with_capacity(self.nevent);

        loop {
            WORKER_EVENT_LOOP.increment();

            self.storage.expire();

            // get events with timeout
            if self.poll.poll(&mut events, self.timeout).is_err() {
                error!("Error polling");
            }

            let count = events.iter().count();
            WORKER_EVENT_TOTAL.add(count as _);
            if count == self.nevent {
                WORKER_EVENT_MAX_REACHED.increment();
            } else {
                WORKER_EVENT_DEPTH.increment(
                    common::time::Instant::<common::time::Nanoseconds<u64>>::now(),
                    count as _,
                    1,
                );
            }

            common::time::refresh_clock();

            // process all events
            for event in events.iter() {
                match event.token() {
                    WAKER_TOKEN => {
                        self.handle_new_sessions();

                        // check if we received any signals from the admin thread
                        while let Some(signal) = self.signal_queue.try_recv() {
                            match signal.into_inner() {
                                Signal::FlushAll => {
                                    warn!("received flush_all");
                                    self.storage.clear();
                                }
                                Signal::Shutdown => {
                                    // if we received a shutdown, we can return
                                    // and stop processing events
                                    return;
                                }
                            }
                        }
                    }
                    _ => {
                        self.handle_event(event);
                    }
                }
            }
        }
    }

    fn handle_new_sessions(&mut self) {
        while let Some(session) = self.session_queue.try_recv().map(|v| v.into_inner()) {
            let pending = session.read_pending();
            trace!(
                "new session: {:?} with {} bytes pending in read buffer",
                session,
                pending
            );

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
            if let Ok(session) = self.poll.get_mut_session(token) {
                session.set_timestamp(
                    common::time::Instant::<common::time::Nanoseconds<u64>>::recent(),
                );
            }
            let _ = self.do_read(token);
        }

        if let Ok(session) = self.poll.get_mut_session(token) {
            if session.read_pending() > 0 {
                trace!(
                    "session: {:?} has {} bytes pending in read buffer",
                    session,
                    session.read_pending()
                );
            }
            if session.write_pending() > 0 {
                trace!(
                    "session: {:?} has {} bytes pending in write buffer",
                    session,
                    session.read_pending()
                );
            }
        }
    }
}

impl<Storage, Server, Request, Response> EventLoop
    for SingleWorker<Storage, Server, Request, Response>
where
    Server: service_common::Server<Request, Response>,
    Response: Compose,
    Storage: Execute<Request, Response> + EntryStore,
{
    fn handle_data(&mut self, token: Token) -> Result<(), std::io::Error> {
        if let Ok(session) = self.poll.get_mut_session(token) {
            loop {
                if session.write_capacity() == 0 {
                    // if the write buffer is over-full, skip processing
                    break;
                }
                match self.server.recv(session.buffer()) {
                    Ok(parsed_request) => {
                        trace!("parsed request for sesion: {:?}", session);
                        PROCESS_REQ.increment();
                        let consumed = parsed_request.consumed();
                        let request = parsed_request.into_inner();
                        session.consume(consumed);

                        let (request, response) = self.storage.execute(request);
                        let should_hangup = response.should_hangup();

                        trace!("composing response for session: {:?}", session);
                        self.server.send(session, request, response);

                        session.finalize_response();
                        if should_hangup {
                            return Err(std::io::Error::new(
                                std::io::ErrorKind::Other,
                                "response requires hangup",
                            ));
                        }
                    }
                    Err(ParseError::Incomplete) => {
                        trace!("incomplete request for session: {:?}", session);
                        break;
                    }
                    Err(_) => {
                        debug!("bad request for session: {:?}", session);
                        trace!("session: {:?} read buffer: {:?}", session, session.buffer());
                        self.handle_error(token);
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::Other,
                            "bad request",
                        ));
                    }
                }
            }
            // if we have pending writes, we should attempt to flush the session
            // now. if we still have pending bytes, we should re-register to
            // remove the read interest.
            if session.write_pending() > 0 {
                let _ = session.flush();
                if session.write_pending() > 0 {
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
