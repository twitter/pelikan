// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! The multi-threaded worker, which is used when there are multiple worker
//! threads configured. This worker parses buffers to produce requests, sends
//! the requests to the storage worker. Responses from the storage worker are
//! then serialized onto the session buffer.

use super::*;
use crate::poll::Poll;
use crate::QUEUE_RETRIES;
use common::signal::Signal;
use config::WorkerConfig;
use core::marker::PhantomData;
use core::time::Duration;
use entrystore::EntryStore;
use mio::event::Event;
use mio::{Events, Token, Waker};
use protocol_common::{Compose, Execute, Parse, ParseError};
use queues::TrackedItem;
use session::Session;
use std::io::{BufRead, Write};
use std::sync::Arc;

const WAKER_TOKEN: Token = Token(usize::MAX);
const STORAGE_THREAD_ID: usize = 0;

/// A builder for the request/response worker which communicates to the storage
/// thread over a queue.
pub struct MultiWorkerBuilder<Storage, Parser, Request, Response> {
    nevent: usize,
    parser: Parser,
    poll: Poll,
    timeout: Duration,
    _storage: PhantomData<Storage>,
    _request: PhantomData<Request>,
    _response: PhantomData<Response>,
}

impl<Storage, Parser, Request, Response> MultiWorkerBuilder<Storage, Parser, Request, Response> {
    /// Create a new builder from the provided config and parser.
    pub fn new<T: WorkerConfig>(config: &T, parser: Parser) -> Result<Self, std::io::Error> {
        let poll = Poll::new().map_err(|e| {
            error!("{}", e);
            std::io::Error::new(std::io::ErrorKind::Other, "Failed to create epoll instance")
        })?;

        Ok(Self {
            poll,
            nevent: config.worker().nevent(),
            timeout: Duration::from_millis(config.worker().timeout() as u64),
            _request: PhantomData,
            _response: PhantomData,
            _storage: PhantomData,
            parser,
        })
    }

    /// Get the waker that is registered to the epoll instance.
    pub(crate) fn waker(&self) -> Arc<Waker> {
        self.poll.waker()
    }

    /// Converts the builder into a `MultiWorker` by providing the queues that
    /// are necessary for communication between components.
    pub fn build(
        self,
        signal_queue: Queues<(), Signal>,
        session_queue: Queues<(), Session>,
        storage_queue: Queues<
            TokenWrapper<Request>,
            WrappedResult<Request, Response>,
        >,
    ) -> MultiWorker<Storage, Parser, Request, Response> {
        MultiWorker {
            nevent: self.nevent,
            parser: self.parser,
            poll: self.poll,
            timeout: self.timeout,
            signal_queue,
            _storage: PhantomData,
            storage_queue,
            session_queue,
        }
    }
}

/// Represents a finalized request/response worker which is ready to be run.
pub struct MultiWorker<Storage, Parser, Request, Response> {
    nevent: usize,
    parser: Parser,
    poll: Poll,
    timeout: Duration,
    session_queue: Queues<(), Session>,
    signal_queue: Queues<(), Signal>,
    _storage: PhantomData<Storage>,
    storage_queue: Queues<TokenWrapper<Request>, WrappedResult<Request, Response>>,
}

impl<Storage, Parser, Request, Response> MultiWorker<Storage, Parser, Request, Response>
where
    Parser: Parse<Request>,
    Response: Compose,
    Storage: Execute<Request, Response> + EntryStore,
{
    /// Run the worker in a loop, handling new events.
    pub fn run(&mut self) {
        // these are buffers which are re-used in each loop iteration to receive
        // events and queue messages
        let mut events = Events::with_capacity(self.nevent);
        let mut responses = Vec::with_capacity(QUEUE_CAPACITY);
        let mut sessions = Vec::with_capacity(QUEUE_CAPACITY);

        loop {
            WORKER_EVENT_LOOP.increment();

            // get events with timeout
            if self.poll.poll(&mut events, self.timeout).is_err() {
                error!("Error polling");
            }

            WORKER_EVENT_TOTAL.add(events.iter().count() as _);

            common::time::refresh_clock();

            // process all events
            for event in events.iter() {
                match event.token() {
                    WAKER_TOKEN => {
                        self.handle_new_sessions(&mut sessions);
                        self.handle_storage_queue(&mut responses);

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
                        self.handle_event(event);
                    }
                }
            }

            // wakes the storage thread if necessary
            let _ = self.storage_queue.wake();
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

    fn handle_session_read(&mut self, token: Token) -> Result<(), std::io::Error> {
        let session = self.poll.get_mut_session(token)?;
        match self.parser.parse(session.buffer()) {
            Ok(request) => {
                let consumed = request.consumed();
                let request = request.into_inner();
                trace!("parsed request for sesion: {:?}", session);
                session.consume(consumed);
                let mut message = TokenWrapper::new(request, token);

                for retry in 0..QUEUE_RETRIES {
                    if let Err(m) = self.storage_queue.try_send_to(STORAGE_THREAD_ID, message) {
                        if (retry + 1) == QUEUE_RETRIES {
                            error!("queue full trying to send message to storage thread");
                            let _ = self.poll.close_session(token);
                        }
                        // try to wake storage thread
                        let _ = self.storage_queue.wake();
                        message = m;
                    } else {
                        break;
                    }
                }
                Ok(())
            }
            Err(ParseError::Incomplete) => {
                trace!("incomplete request for session: {:?}", session);
                Err(std::io::Error::new(
                    std::io::ErrorKind::WouldBlock,
                    "incomplete request",
                ))
            }
            Err(_) => {
                debug!("bad request for session: {:?}", session);
                trace!("session: {:?} read buffer: {:?}", session, session.buffer());
                let _ = self.poll.close_session(token);
                Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "bad request",
                ))
            }
        }
    }

    fn handle_storage_queue(
        &mut self,
        responses: &mut Vec<TrackedItem<WrappedResult<Request, Response>>>,
    ) {
        trace!("handling event for storage queue");
        // process all storage queue responses
        self.storage_queue.try_recv_all(responses);

        for message in responses.drain(..).map(|v| v.into_inner()) {
            let token = message.token();
            let mut reregister = false;
            if let Ok(session) = self.poll.get_mut_session(token) {
                let result = message.into_inner();
                trace!("composing response for session: {:?}", session);
                result.compose(session);
                session.finalize_response();
                // if we have pending writes, we should attempt to flush the session
                // now. if we still have pending bytes, we should re-register to
                // remove the read interest.
                if session.write_pending() > 0 {
                    let _ = session.flush();
                    if session.write_pending() > 0 {
                        reregister = true;
                    }
                }
                if session.read_pending() > 0 && self.handle_session_read(token).is_ok() {
                    let _ = self.storage_queue.wake();
                }
            }
            if reregister {
                self.poll.reregister(token);
            }
        }
        let _ = self.storage_queue.wake();
    }

    fn handle_new_sessions(&mut self, sessions: &mut Vec<TrackedItem<Session>>) {
        self.session_queue.try_recv_all(sessions);
        for session in sessions.drain(..).map(|v| v.into_inner()) {
            let pending = session.read_pending();
            trace!(
                "new session: {:?} with {} bytes pending in read buffer",
                session,
                pending
            );

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
}

impl<Storage, Parser, Request, Response> EventLoop
    for MultiWorker<Storage, Parser, Request, Response>
where
    Parser: Parse<Request>,
    Response: Compose,
    Storage: Execute<Request, Response> + EntryStore,
{
    fn handle_data(&mut self, token: Token) -> Result<(), std::io::Error> {
        let _ = self.handle_session_read(token);
        Ok(())
    }

    fn poll(&mut self) -> &mut Poll {
        &mut self.poll
    }
}
