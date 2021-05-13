// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! The multi-worker which is used when multiple worker threads are enabled in
//! the config. Parsed requests are dispatched to the [`Storage`] thread for
//! handling.

use crossbeam_channel::{Receiver, Sender};
use metrics::Stat;
use mio::event::Event;
use rtrb::PushError;

use crate::common::Message;
use crate::event_loop::EventLoop;
use crate::protocol::data::*;
use crate::session::*;
use crate::storage::*;
use crate::*;

use std::convert::TryInto;
use std::sync::Arc;

// TODO(bmartin): this *should* be plenty safe, the queue should rarely ever be
// full, and a single wakeup should drain at least one message and make room for
// the request. A stat to prove that this is sufficient would be good.
const QUEUE_RETRIES: usize = 3;

/// A `MultiWorker` handles events on `Session`s and routes storage requests to
/// the `Storage` thread.
pub struct MultiWorker {
    config: Arc<Config>,
    message_receiver: Receiver<Message>,
    message_sender: Sender<Message>,
    poll: Poll,
    session_receiver: Receiver<Session>,
    session_sender: Sender<Session>,
    sessions: Slab<Session>,
    storage_queue: StorageQueue,
    wake_storage: bool,
}

impl MultiWorker {
    /// Create a new `Worker` which will get new `Session`s from the MPSC queue
    pub fn new(config: Arc<Config>, storage: &mut Storage) -> Result<Self, std::io::Error> {
        let poll = Poll::new().map_err(|e| {
            error!("{}", e);
            std::io::Error::new(std::io::ErrorKind::Other, "Failed to create epoll instance")
        })?;
        let sessions = Slab::<Session>::new();

        let waker = Waker::new(poll.registry(), Token(usize::MAX)).unwrap();
        let storage_queue = storage.add_queue(waker);

        let (session_sender, session_receiver) = crossbeam_channel::bounded(128);
        let (message_sender, message_receiver) = crossbeam_channel::bounded(128);

        Ok(Self {
            config,
            poll,
            message_receiver,
            message_sender,
            session_receiver,
            session_sender,
            sessions,
            storage_queue,
            wake_storage: false,
        })
    }

    /// Run the `Worker` in a loop, handling new session events
    pub fn run(&mut self) {
        let mut events = Events::with_capacity(self.config.worker().nevent());
        let timeout = Some(std::time::Duration::from_millis(
            self.config.worker().timeout() as u64,
        ));

        loop {
            increment_counter!(&Stat::WorkerEventLoop);

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
                self.handle_event(&event);
            }

            // if we sent any messages to the storage thread, we need to wake it
            if self.wake_storage && self.storage_queue.wake().is_ok() {
                self.wake_storage = false;
            }

            // poll queue to receive new sessions
            self.handle_new_sessions();

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

    fn handle_event(&mut self, event: &Event) {
        let token = event.token();

        if token.0 == usize::MAX {
            self.handle_storage_queue();
            return;
        }

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

    fn handle_session_read(
        session: &mut Session,
        poll: &Poll,
        storage_queue: &mut StorageQueue,
    ) -> bool {
        match MemcacheParser::parse(&mut session.read_buffer) {
            Ok(request) => {
                let mut message = RequestMessage {
                    request,
                    token: session.token(),
                };
                for retry in 0..QUEUE_RETRIES {
                    if let Err(PushError::Full(m)) = storage_queue.try_send(message) {
                        if (retry + 1) == QUEUE_RETRIES {
                            error!("queue full trying to send message to storage thread");
                            if session.deregister(poll).is_err() {
                                error!("Error deregistering");
                            }
                            session.close()
                        }
                        // try to wake storage thread
                        let _ = storage_queue.wake();
                        message = m;
                    } else {
                        break;
                    }
                }
                true
            }
            Err(ParseError::Incomplete) => false,
            Err(_) => {
                if session.deregister(poll).is_err() {
                    error!("Error deregistering");
                }
                session.close();
                false
            }
        }
    }

    fn handle_storage_queue(&mut self) {
        trace!("handling event for storage queue");
        // process all storage queue responses
        while let Ok(message) = self.storage_queue.try_recv() {
            let token = message.token;
            if let Some(session) = self.sessions.get_mut(token.0) {
                // session.write_buffer = Some(message.buffer);
                message.response.serialize(&mut session.write_buffer);
                if session.write_pending() > 0 {
                    match session.flush() {
                        Ok(_) => {
                            if session.write_pending() > 0 {
                                let _ = session.reregister(&self.poll);
                            }
                        }
                        Err(e) => {
                            error!("error flushing: {}", e);
                        }
                    }
                }

                if session.read_pending() > 0
                    && Self::handle_session_read(session, &self.poll, &mut self.storage_queue)
                {
                    self.wake_storage = true;
                }
            }
        }
    }

    fn handle_new_sessions(&mut self) {
        while let Ok(mut session) = self.session_receiver.try_recv() {
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

    pub fn message_sender(&self) -> Sender<Message> {
        self.message_sender.clone()
    }

    pub fn session_sender(&self) -> Sender<Session> {
        self.session_sender.clone()
    }
}

impl EventLoop for MultiWorker {
    fn get_mut_session(&mut self, token: Token) -> Option<&mut Session> {
        self.sessions.get_mut(token.0)
    }

    fn handle_data(&mut self, token: Token) -> Result<(), ()> {
        trace!("handling request for session: {}", token.0);
        if let Some(session) = self.sessions.get_mut(token.0) {
            if session.write_pending() < MIN_BUFFER_SIZE
                && Self::handle_session_read(session, &self.poll, &mut self.storage_queue)
            {
                self.wake_storage = true;
            }

            if session.write_pending() > 0 && session.flush().is_ok() && session.write_pending() > 0
            {
                self.reregister(token);
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
