// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::*;
use common::signal::Signal;
use config::proxy::BackendConfig;
use core::marker::PhantomData;
use core::time::Duration;
use mio::Waker;
use poll::*;
use protocol_common::*;
use queues::Queues;
use queues::TrackedItem;
use session::Session;
use std::sync::Arc;

use rustcommon_metrics::*;

const KB: usize = 1024;

const SESSION_BUFFER_MIN: usize = 16 * KB;
const SESSION_BUFFER_MAX: usize = 1024 * KB;

counter!(BACKEND_EVENT_ERROR);
counter!(BACKEND_EVENT_READ);
counter!(BACKEND_EVENT_WRITE);
counter!(
    BACKEND_EVENT_MAX_REACHED,
    "the number of times the maximum number of events was returned"
);
heatmap!(BACKEND_EVENT_MAX_REACHED_DEPTH, 1_000_000);

pub const QUEUE_RETRIES: usize = 3;

pub struct BackendWorkerBuilder<Parser, Request, Response> {
    poll: Poll,
    parser: Parser,
    free_queue: VecDeque<Token>,
    nevent: usize,
    timeout: Duration,
    _request: PhantomData<Request>,
    _response: PhantomData<Response>,
}

impl<Parser, Request, Response> BackendWorkerBuilder<Parser, Request, Response> {
    pub fn new<T: BackendConfig>(config: &T, parser: Parser) -> Result<Self> {
        let config = config.backend();

        let mut poll = Poll::new()?;

        let server_endpoints = config.socket_addrs()?;

        let mut free_queue = VecDeque::with_capacity(server_endpoints.len() * config.poolsize());

        for addr in server_endpoints {
            for _ in 0..config.poolsize() {
                let connection = std::net::TcpStream::connect(addr).expect("failed to connect");
                connection
                    .set_nonblocking(true)
                    .expect("failed to set non-blocking");
                let connection = TcpStream::from_std(connection);
                let session = Session::plain_with_capacity(
                    session::TcpStream::try_from(connection).expect("failed to convert"),
                    SESSION_BUFFER_MIN,
                    SESSION_BUFFER_MAX,
                );
                if let Ok(token) = poll.add_session(session) {
                    println!("new backend connection with token: {}", token.0);
                    free_queue.push_back(token);
                }
            }
        }

        Ok(Self {
            poll,
            free_queue,
            parser,
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
        queues: Queues<TokenWrapper<Response>, TokenWrapper<Request>>,
    ) -> BackendWorker<Parser, Request, Response> {
        BackendWorker {
            poll: self.poll,
            free_queue: self.free_queue,
            signal_queue,
            queues,
            parser: self.parser,
            nevent: self.nevent,
            timeout: self.timeout,
        }
    }
}

pub struct BackendWorker<Parser, Request, Response> {
    poll: Poll,
    queues: Queues<TokenWrapper<Response>, TokenWrapper<Request>>,
    free_queue: VecDeque<Token>,
    signal_queue: Queues<(), Signal>,
    parser: Parser,
    nevent: usize,
    timeout: Duration,
}

impl<Parser, Request, Response> BackendWorker<Parser, Request, Response>
where
    Request: Compose,
    Parser: Parse<Response>,
{
    #[allow(clippy::match_single_binding)]
    pub fn run(mut self) {
        let mut events = Events::with_capacity(self.nevent);
        let mut requests = Vec::with_capacity(self.nevent);
        loop {
            let _ = self.poll.poll(&mut events, self.timeout);
            for event in &events {
                match event.token() {
                    WAKER_TOKEN => {
                        self.handle_waker(&mut requests);
                        if !requests.is_empty() {
                            let _ = self.poll.waker().wake();
                        }
                    }
                    _ => {
                        self.handle_event(event);
                    }
                }
            }
            let count = events.iter().count();
            if count == self.nevent {
                BACKEND_EVENT_MAX_REACHED.increment();
            }
            BACKEND_EVENT_MAX_REACHED_DEPTH.increment(
                common::time::Instant::<common::time::Nanoseconds<u64>>::now(),
                count as _,
                1,
            );
            let _ = self.queues.wake();
        }
    }

    fn handle_event(&mut self, event: &Event) {
        let token = event.token();

        // handle error events first
        if event.is_error() {
            BACKEND_EVENT_ERROR.increment();
            self.handle_error(token);
        }

        // handle write events before read events to reduce write buffer
        // growth if there is also a readable event
        if event.is_writable() {
            BACKEND_EVENT_WRITE.increment();
            self.do_write(token);
        }

        // read events are handled last
        if event.is_readable() {
            BACKEND_EVENT_READ.increment();
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

    pub fn handle_waker(&mut self, requests: &mut Vec<TrackedItem<TokenWrapper<Request>>>) {
        // try to get requests from the queue if we don't already
        // have a backlog
        if requests.is_empty() {
            self.queues.try_recv_all(requests);
        }

        // as long as we have free backend connections and we
        // have requests from the most recent read of the queue
        // we can dispatch requests
        while !self.free_queue.is_empty() && !requests.is_empty() {
            let backend_token = self.free_queue.pop_front().unwrap();
            let request = requests.remove(0);

            // check if this token is still a valid connection
            if let Ok(session) = self.poll.get_mut_session(backend_token) {
                if session.token.is_none() && session.sender.is_none() {
                    let sender = request.sender();
                    let request = request.into_inner();
                    let token = request.token();
                    let request = request.into_inner();

                    session.sender = Some(sender);
                    session.token = Some(token);
                    request.compose(&mut session.session);
                    session.session.finalize_response();

                    if session.session.write_pending() > 0 {
                        let _ = session.session.flush();
                        if session.session.write_pending() > 0 {
                            self.poll.reregister(token);
                        }
                    }
                }
            }

            self.poll.reregister(backend_token);
        }
    }

    fn handle_session_read(&mut self, token: Token) -> Result<()> {
        let s = self.poll.get_mut_session(token)?;
        let session = &mut s.session;
        match self.parser.parse(session.buffer()) {
            Ok(response) => {
                let consumed = response.consumed();
                let response = response.into_inner();
                session.consume(consumed);

                let fe_worker = s.sender.take().unwrap();
                let client_token = s.token.take().unwrap();

                let mut message = TokenWrapper::new(response, client_token);

                for retry in 0..QUEUE_RETRIES {
                    if let Err(m) = self.queues.try_send_to(fe_worker, message) {
                        if (retry + 1) == QUEUE_RETRIES {
                            error!("queue full trying to send response to frontend");
                            let _ = self.poll.close_session(token);
                        }
                        // try to wake frontend thread
                        let _ = self.queues.wake();
                        message = m;
                    } else {
                        break;
                    }
                }

                self.free_queue.push_back(token);

                let _ = self.queues.wake();

                Ok(())
            }
            Err(ParseError::Incomplete) => {
                trace!("incomplete response for session: {:?}", session);
                Err(std::io::Error::new(
                    std::io::ErrorKind::WouldBlock,
                    "incomplete response",
                ))
            }
            Err(_) => {
                debug!("bad response for session: {:?}", session);
                trace!("session: {:?} read buffer: {:?}", session, session.buffer());
                let _ = self.poll.close_session(token);
                Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "bad response",
                ))
            }
        }
    }
}

impl<Parser, Request, Response> EventLoop for BackendWorker<Parser, Request, Response>
where
    Request: Compose,
    Parser: Parse<Response>,
{
    fn handle_data(&mut self, token: Token) -> Result<()> {
        let _ = self.handle_session_read(token);
        Ok(())
    }

    fn poll(&mut self) -> &mut poll::Poll {
        &mut self.poll
    }
}
