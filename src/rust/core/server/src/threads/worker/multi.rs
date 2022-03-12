// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! The multi-threaded worker, which is used when there are multiple worker
//! threads configured. This worker parses buffers to produce requests, sends
//! the requests to the storage worker. Responses from the storage worker are
//! then serialized onto the session buffer.

use super::EventLoop;
use super::*;
use crate::poll::Poll;
use crate::threads::worker::StorageWorker;
use crate::threads::worker::TokenWrapper;
use common::signal::Signal;
use common::time::Instant;
use config::WorkerConfig;
use core::marker::PhantomData;
use core::time::Duration;
use entrystore::EntryStore;
use mio::event::Event;
use mio::{Events, Token, Waker};
use protocol_common::{Compose, Execute, Parse, ParseError};
use queues::{QueuePair, QueuePairs, SendError};
use session::Session;
use std::io::{BufRead, Write};
use std::sync::Arc;

// TODO(bmartin): this *should* be plenty safe, the queue should rarely ever be
// full, and a single wakeup should drain at least one message and make room for
// the request. A stat to prove that this is sufficient would be good.
const QUEUE_RETRIES: usize = 3;

const WAKER_TOKEN: usize = usize::MAX;

/// A `MultiWorker` handles events on `Session`s and routes storage requests to
/// the `Storage` thread.
pub struct MultiWorker<Storage, Parser, Request, Response>
where
    Parser: Parse<Request>,
    Response: Compose,
{
    signal_queue: QueuePairs<Signal, Signal>,
    poll: Poll,
    nevent: usize,
    timeout: Duration,
    session_queue: QueuePairs<(), Session>,
    storage_queue: QueuePair<TokenWrapper<Request>, TokenWrapper<Option<Response>>>,
    wake_storage: bool,
    _storage: PhantomData<Storage>,
    parser: Parser,
}

impl<Storage, Parser, Request, Response> MultiWorker<Storage, Parser, Request, Response>
where
    Parser: Parse<Request> + Send,
    Request: Send,
    Response: Compose + Send,
    Storage: Execute<Request, Response> + EntryStore + Send,
{
    /// Create a new `Worker` which will get new `Session`s from the MPSC queue
    pub fn new<T: WorkerConfig>(
        config: &T,
        storage: &mut StorageWorker<Storage, Request, Response>,
        parser: Parser,
    ) -> Result<Self, std::io::Error> {
        let poll = Poll::new().map_err(|e| {
            error!("{}", e);
            std::io::Error::new(std::io::ErrorKind::Other, "Failed to create epoll instance")
        })?;

        let storage_queue = storage.add_queue(poll.waker());

        let session_queue = QueuePairs::new(Some(poll.waker()));
        let signal_queue = QueuePairs::new(Some(poll.waker()));

        Ok(Self {
            poll,
            nevent: config.worker().nevent(),
            timeout: Duration::from_millis(config.worker().timeout() as u64),
            signal_queue,
            session_queue,
            storage_queue,
            wake_storage: false,
            _storage: PhantomData,
            parser,
        })
    }

    /// Run the `Worker` in a loop, handling new session events
    pub fn run(&mut self) {
        let mut events = Events::with_capacity(self.nevent);

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
                    Token(WAKER_TOKEN) => {
                        self.handle_new_sessions();
                        self.handle_storage_queue();

                        #[allow(clippy::never_loop)]
                        // check if we received any signals from the admin thread
                        while let Ok(signal) = self.signal_queue.recv_from(0) {
                            match signal {
                                Signal::Shutdown => {
                                    // if we received a shutdown, we can return
                                    // and stop processing events
                                    return;
                                }
                            }
                        }
                    }
                    Token(_) => {
                        self.handle_event(event);
                    }
                }
            }

            // if we sent any messages to the storage thread, we need to wake it
            if self.wake_storage && self.storage_queue.wake().is_ok() {
                trace!("sent wake to storage");
                self.wake_storage = false;
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
                session.set_timestamp(Instant::<Nanoseconds<u64>>::recent());
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
                    if let Err(SendError::Full(m)) = self.storage_queue.try_send(message) {
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

    fn handle_storage_queue(&mut self) {
        trace!("handling event for storage queue");
        // process all storage queue responses
        while let Ok(message) = self.storage_queue.try_recv() {
            let token = message.token();
            let mut reregister = false;
            if let Ok(session) = self.poll.get_mut_session(token) {
                if let Some(response) = message.into_inner() {
                    trace!("composing response for session: {:?}", session);
                    response.compose(session);
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
                }

                if session.read_pending() > 0 && self.handle_session_read(token).is_ok() {
                    self.wake_storage = true;
                }
            }
            if reregister {
                self.poll.reregister(token);
            }
        }
    }

    fn handle_new_sessions(&mut self) {
        while let Ok(session) = self.session_queue.recv_from(0) {
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

    pub fn session_sender(&mut self, waker: Arc<Waker>) -> QueuePair<Session, ()> {
        self.session_queue.new_pair(65536, Some(waker))
    }

    pub fn signal_queue(&mut self) -> QueuePair<Signal, Signal> {
        self.signal_queue.new_pair(128, None)
    }
}

impl<Storage, Parser, Request, Response> EventLoop
    for MultiWorker<Storage, Parser, Request, Response>
where
    Parser: Parse<Request> + Send,
    Request: Send,
    Response: Compose + Send,
    Storage: Execute<Request, Response> + EntryStore + Send,
{
    fn handle_data(&mut self, token: Token) -> Result<(), std::io::Error> {
        if self.handle_session_read(token).is_ok() {
            self.wake_storage = true;
        }
        Ok(())
    }

    fn poll(&mut self) -> &mut Poll {
        &mut self.poll
    }
}
