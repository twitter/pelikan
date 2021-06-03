// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! The multi-threaded worker, which is used when there are multiple worker
//! threads configured. This worker parses buffers to produce requests, sends
//! the requests to the storage worker. Responses from the storage worker are
//! then serialized onto the session buffer.

use super::EventLoop;
use crate::poll::Poll;
use crate::threads::worker::StorageWorker;
use crate::threads::worker::TokenWrapper;
use common::signal::Signal;
use config::WorkerConfig;
use core::marker::PhantomData;
use core::time::Duration;
use entrystore::EntryStore;
use metrics::Stat;
use mio::event::Event;
use mio::{Events, Token, Waker};
use protocol::{Compose, Execute, Parse, ParseError};
use queues::{QueuePair, QueuePairs, SendError};
use session::Session;
use std::convert::TryInto;
use std::io::{BufRead, Write};
use std::sync::Arc;

// TODO(bmartin): this *should* be plenty safe, the queue should rarely ever be
// full, and a single wakeup should drain at least one message and make room for
// the request. A stat to prove that this is sufficient would be good.
const QUEUE_RETRIES: usize = 3;

const WAKER_TOKEN: usize = usize::MAX;

/// A `MultiWorker` handles events on `Session`s and routes storage requests to
/// the `Storage` thread.
pub struct MultiWorker<Storage, Request, Response>
where
    Request: Parse,
    Response: protocol::Compose,
{
    signal_queue: QueuePairs<(), Signal>,
    poll: Poll,
    nevent: usize,
    timeout: Duration,
    session_queue: QueuePairs<(), Session>,
    storage_queue: QueuePair<TokenWrapper<Request>, TokenWrapper<Option<Response>>>,
    wake_storage: bool,
    _storage: PhantomData<Storage>,
}

impl<Storage, Request, Response> MultiWorker<Storage, Request, Response>
where
    Request: Parse + Send,
    Response: Compose + Send,
    Storage: Execute<Request, Response> + EntryStore + Send,
{
    /// Create a new `Worker` which will get new `Session`s from the MPSC queue
    pub fn new(
        config: &WorkerConfig,
        storage: &mut StorageWorker<Storage, Request, Response>,
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
            nevent: config.nevent(),
            timeout: Duration::from_millis(config.timeout() as u64),
            signal_queue,
            session_queue,
            storage_queue,
            wake_storage: false,
            _storage: PhantomData,
        })
    }

    /// Run the `Worker` in a loop, handling new session events
    pub fn run(&mut self) {
        let mut events = Events::with_capacity(self.nevent);

        loop {
            increment_counter!(&Stat::WorkerEventLoop);

            // get events with timeout
            if self.poll.poll(&mut events, self.timeout).is_err() {
                error!("Error polling");
            }

            increment_counter_by!(
                &Stat::WorkerEventTotal,
                events.iter().count().try_into().unwrap(),
            );

            // process all events
            for event in events.iter() {
                match event.token() {
                    Token(WAKER_TOKEN) => {
                        self.handle_new_sessions();
                        self.handle_storage_queue();

                        #[allow(clippy::never_loop)]
                        while let Ok(signal) = self.signal_queue.recv_from(0) {
                            match signal {
                                Signal::Shutdown => {
                                    return;
                                }
                            }
                        }
                    }
                    Token(_) => {
                        self.handle_event(&event);
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

    fn handle_session_read(
        &mut self,
        token: Token,
    ) -> Result<(), std::io::Error> {
        match Request::parse(self.poll.get_mut_session(token)?.buffer()) {
            Ok(request) => {
                let consumed = request.consumed();
                let request = request.into_inner();
                self.poll.get_mut_session(token)?.consume(consumed);
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
                Err(std::io::Error::new(std::io::ErrorKind::WouldBlock, "incomplete request"))
            }
            Err(_) => {
                let _ = self.poll.close_session(token);
                Err(std::io::Error::new(std::io::ErrorKind::Other, "bad request"))
            }
        }
    }

    fn handle_storage_queue(&mut self) {
        trace!("handling event for storage queue");
        // process all storage queue responses
        while let Ok(message) = self.storage_queue.try_recv() {
            let token = message.token();
            let mut reregister = false;
            if let Ok(mut session) = self.poll.get_mut_session(token) {
                if let Some(response) = message.into_inner() {
                    response.compose(&mut session);
                    if session.write_pending() > 0 {
                        match session.flush() {
                            Ok(_) => {
                                if session.write_pending() > 0 {
                                    reregister = true;
                                }
                            }
                            Err(e) => {
                                error!("error flushing: {}", e);
                            }
                        }
                    }
                }

                if session.read_pending() > 0
                    && self.handle_session_read(token).is_ok()
                {
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
            trace!("{} bytes pending in rx buffer for new session", pending);

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

    pub fn signal_queue(&mut self) -> QueuePair<Signal, ()> {
        self.signal_queue.new_pair(128, None)
    }
}

impl<Storage, Request, Response> EventLoop for MultiWorker<Storage, Request, Response>
where
    Request: Parse + Send,
    Response: Compose + Send,
    Storage: Execute<Request, Response> + EntryStore + Send,
{
    fn handle_data(&mut self, token: Token) -> Result<(), std::io::Error> {
        trace!("handling request for session: {}", token.0);
        let write_capacity = self.poll.get_mut_session(token)?.write_capacity();
        if write_capacity > 0 && self.handle_session_read(token).is_ok() {
            self.wake_storage = true;
        }

        let write_pending = self.poll.get_mut_session(token)?.write_pending();
        if write_pending > 0 {
            {
                let session = self.poll.get_mut_session(token)?;
                let _ = session.flush();
            }
            let write_pending = self.poll.get_mut_session(token)?.write_pending();
            if write_pending > 0 {
                self.poll.reregister(token);
            }
        }
        Ok(())
    }

    fn poll(&mut self) -> &mut Poll {
        &mut self.poll
    }
}
