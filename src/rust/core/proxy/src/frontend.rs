// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use common::signal::Signal;
use crate::*;
use common::time::Instant;
use core::marker::PhantomData;
use mio::Waker;
use poll::*;
use protocol_common::*;
use queues::Queues;
use session::Session;
use std::sync::Arc;

static_metrics! {
    static FRONTEND_EVENT_ERROR: Counter;
    static FRONTEND_EVENT_READ: Counter;
    static FRONTEND_EVENT_WRITE: Counter;
}

pub const QUEUE_RETRIES: usize = 3;

pub struct FrontendWorkerBuilder<Parser, Request, Response> {
    poll: Poll,
    parser: Parser,
    _request: PhantomData<Request>,
    _response: PhantomData<Response>,
}

impl<Parser, Request, Response> FrontendWorkerBuilder<Parser, Request, Response> {
    pub fn new(parser: Parser) -> Result<Self> {
        Ok(Self {
            poll: Poll::new()?,
            parser,
            _request: PhantomData,
            _response: PhantomData,
        })
    }

    pub fn waker(&self) -> Arc<Waker> {
        self.poll.waker()
    }

    pub fn build(
        self,
        signal_queue: Queues<(), Signal>,
        connection_queues: Queues<(), Session>,
        data_queues: Queues<TokenWrapper<Request>, TokenWrapper<Response>>,
    ) -> FrontendWorker<Parser, Request, Response> {
        FrontendWorker {
            poll: self.poll,
            parser: self.parser,
            signal_queue,
            connection_queues,
            data_queues,
        }
    }
}

pub struct FrontendWorker<Parser, Request, Response> {
    poll: Poll,
    parser: Parser,
    signal_queue: Queues<(), Signal>,
    connection_queues: Queues<(), Session>,
    data_queues: Queues<TokenWrapper<Request>, TokenWrapper<Response>>,
}

impl<Parser, Request, Response> FrontendWorker<Parser, Request, Response>
where
    Parser: Parse<Request>,
    Response: Compose,
{
    #[allow(clippy::match_single_binding)]
    pub fn run(mut self) {
        let mut events = Events::with_capacity(1024);
        let mut sessions = Vec::with_capacity(1024);
        let mut responses = Vec::with_capacity(1024);
        loop {
            let _ = self
                .poll
                .poll(&mut events, core::time::Duration::from_millis(100));
            for event in &events {
                match event.token() {
                    WAKER_TOKEN => {
                        self.connection_queues.try_recv_all(&mut sessions);
                        for session in sessions.drain(..).map(|v| v.into_inner()) {
                            if self.poll.add_session(session).is_ok() {
                                trace!("frontend registered new session");
                            } else {
                                warn!("frontend failed to register new session");
                            }
                        }
                        self.data_queues.try_recv_all(&mut responses);
                        for response in responses.drain(..).map(|v| v.into_inner()) {
                            let token = response.token();
                            let response = response.into_inner();
                            if let Ok(session) = self.poll.get_mut_session(token) {
                                response.compose(&mut session.session);
                                session.session.finalize_response();

                                // if we have pending writes, we should attempt to flush the session
                                // now. if we still have pending bytes, we should re-register to
                                // remove the read interest.
                                if session.session.write_pending() > 0 {
                                    let _ = session.session.flush();
                                    if session.session.write_pending() > 0 {
                                        self.poll.reregister(token);
                                    }
                                }
                            }
                        }
                    }
                    _ => {
                        self.handle_event(event);
                    }
                }
            }
            let _ = self.data_queues.wake();
        }
    }

    fn handle_event(&mut self, event: &Event) {
        let token = event.token();

        // handle error events first
        if event.is_error() {
            FRONTEND_EVENT_ERROR.increment();
            self.handle_error(token);
        }

        // handle write events before read events to reduce write buffer
        // growth if there is also a readable event
        if event.is_writable() {
            FRONTEND_EVENT_WRITE.increment();
            self.do_write(token);
        }

        // read events are handled last
        if event.is_readable() {
            FRONTEND_EVENT_READ.increment();
            if let Ok(session) = self.poll.get_mut_session(token) {
                session.session.set_timestamp(common::time::Instant::<
                    common::time::Nanoseconds<u64>,
                >::recent());
            }
            let _ = self.do_read(token);
        }

        if let Ok(session) = self.poll.get_mut_session(token) {
            if session.session.read_pending() > 0 {
                trace!(
                    "session: {:?} has {} bytes pending in read buffer",
                    session.session,
                    session.session.read_pending()
                );
            }
            if session.session.write_pending() > 0 {
                trace!(
                    "session: {:?} has {} bytes pending in write buffer",
                    session.session,
                    session.session.read_pending()
                );
            }
        }
    }

    fn handle_session_read(&mut self, token: Token) -> Result<()> {
        let s = self.poll.get_mut_session(token)?;
        let session = &mut s.session;
        match self.parser.parse(session.buffer()) {
            Ok(request) => {
                let consumed = request.consumed();
                let request = request.into_inner();
                trace!("parsed request for sesion: {:?}", session);
                session.consume(consumed);
                let mut message = TokenWrapper::new(request, token);

                for retry in 0..QUEUE_RETRIES {
                    if let Err(m) = self.data_queues.try_send_any(message) {
                        if (retry + 1) == QUEUE_RETRIES {
                            warn!("queue full trying to send message to storage thread");
                            let _ = self.poll.close_session(token);
                        }
                        // try to wake storage thread
                        let _ = self.data_queues.wake();
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

    pub fn try_close(&mut self, token: Token) {
        let _ = self.poll.remove_session(token);
    }
}

impl<Parser, Request, Response> EventLoop for FrontendWorker<Parser, Request, Response>
where
    Parser: Parse<Request>,
    Response: Compose,
{
    fn handle_data(&mut self, token: Token) -> Result<()> {
        let _ = self.handle_session_read(token);
        Ok(())
    }

    fn poll(&mut self) -> &mut poll::Poll {
        &mut self.poll
    }
}
