// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::*;
use common::signal::Signal;
use common::time::Instant;
use config::proxy::FrontendConfig;
use core::marker::PhantomData;
use core::time::Duration;
use mio::Waker;
use poll::*;
use protocol_common::*;
use queues::Queues;
use session::Session;
use std::sync::Arc;

use rustcommon_metrics::*;

counter!(FRONTEND_EVENT_ERROR);
counter!(FRONTEND_EVENT_READ);
counter!(FRONTEND_EVENT_WRITE);
counter!(
    FRONTEND_EVENT_MAX_REACHED,
    "the number of times the maximum number of events was returned"
);
heatmap!(FRONTEND_EVENT_DEPTH, 100_000);

pub const QUEUE_RETRIES: usize = 3;

pub struct FrontendWorkerBuilder<Server, Request, Response> {
    poll: Poll,
    server: Server,
    nevent: usize,
    timeout: Duration,
    _request: PhantomData<Request>,
    _response: PhantomData<Response>,
}

impl<Server, Request, Response> FrontendWorkerBuilder<Server, Request, Response> {
    pub fn new<T: FrontendConfig>(config: &T, server: Server) -> Result<Self> {
        let config = config.frontend();

        Ok(Self {
            poll: Poll::new()?,
            server,
            nevent: config.nevent(),
            timeout: Duration::from_millis(config.timeout() as u64),
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
    ) -> FrontendWorker<Server, Request, Response> {
        FrontendWorker {
            poll: self.poll,
            server: self.server,
            nevent: self.nevent,
            timeout: self.timeout,
            signal_queue,
            connection_queues,
            data_queues,
        }
    }
}

pub struct FrontendWorker<Server, Request, Response> {
    poll: Poll,
    server: Server,
    nevent: usize,
    timeout: Duration,
    signal_queue: Queues<(), Signal>,
    connection_queues: Queues<(), Session>,
    data_queues: Queues<TokenWrapper<Request>, TokenWrapper<Response>>,
}

impl<Server, Request, Response> FrontendWorker<Server, Request, Response>
where
    Server: service_common::Server<Request, Response>,
    Response: Compose,
{
    #[allow(clippy::match_single_binding)]
    pub fn run(mut self) {
        let mut events = Events::with_capacity(self.nevent);
        let mut sessions = Vec::with_capacity(self.nevent);
        let mut responses = Vec::with_capacity(self.nevent);
        loop {
            let _ = self.poll.poll(&mut events, self.timeout);
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
            let count = events.iter().count();
            if count == self.nevent {
                FRONTEND_EVENT_MAX_REACHED.increment();
            } else {
                FRONTEND_EVENT_DEPTH.increment(
                    common::time::Instant::<common::time::Nanoseconds<u64>>::now(),
                    count as _,
                    1,
                );
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
                session.session.set_timestamp(rustcommon_time::Instant::<
                    rustcommon_time::Nanoseconds<u64>,
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
        match self.server.recv(session.buffer()) {
            Ok(request) => {
                let consumed = request.consumed();
                let request = request.into_inner();
                trace!("parsed request for sesion: {:?}", session);
                session.consume(consumed);
                let mut message = TokenWrapper::new(request, token);

                for retry in 0..QUEUE_RETRIES {
                    if let Err(m) = self.data_queues.try_send_any(message) {
                        if (retry + 1) == QUEUE_RETRIES {
                            warn!("queue full trying to send message to backend thread");
                            let _ = self.poll.close_session(token);
                        }
                        // try to wake backend thread
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

impl<Server, Request, Response> EventLoop for FrontendWorker<Server, Request, Response>
where
    Server: service_common::Server<Request, Response>,
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
