// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use super::map_result;
use crate::*;
use session_common::ClientSession;
use std::collections::HashMap;
use std::collections::VecDeque;

pub struct BackendWorker<Parser, Request, Response> {
    backlog: VecDeque<(Request, Token)>,
    data_queue: Queues<(Request, Response, Token), (Request, Token)>,
    free_queue: VecDeque<Token>,
    nevent: usize,
    parser: Parser,
    pending: HashMap<Token, Token>,
    poll: Poll,
    sessions: Slab<ClientSession<Parser, Request, Response>>,
    signal_queue: Queues<(), Signal>,
    timeout: Duration,
    waker: Arc<Waker>,
}

impl<Parser, Request, Response> BackendWorker<Parser, Request, Response>
where
    Parser: Parse<Response> + Clone,
    Request: Compose,
{
    /// Return the `Session` to the `Listener` to handle flush/close
    fn close(&mut self, token: Token) {
        if self.sessions.contains(token.0) {
            let mut session = self.sessions.remove(token.0);
            let _ = session.flush();
        }
    }

    /// Handle up to one response for a session
    fn read(&mut self, token: Token) -> Result<()> {
        let session = self
            .sessions
            .get_mut(token.0)
            .ok_or_else(|| Error::new(ErrorKind::Other, "non-existant session"))?;

        // fill the session
        map_result(session.fill())?;

        // process up to one request
        match session.receive() {
            Ok((request, response)) => {
                if let Some(fe_token) = self.pending.remove(&token) {
                    self.free_queue.push_back(token);
                    self.data_queue
                        .try_send_to(0, (request, response, fe_token))
                        .map_err(|_| Error::new(ErrorKind::Other, "data queue is full"))
                } else {
                    panic!("corrupted state");
                }
            }
            Err(e) => map_err(e),
        }
    }

    /// Handle write by flushing the session
    fn write(&mut self, token: Token) -> Result<()> {
        let session = self
            .sessions
            .get_mut(token.0)
            .ok_or_else(|| Error::new(ErrorKind::Other, "non-existant session"))?;

        match session.flush() {
            Ok(_) => Ok(()),
            Err(e) => map_err(e),
        }
    }

    /// Run the worker in a loop, handling new events.
    pub fn run(&mut self) {
        // these are buffers which are re-used in each loop iteration to receive
        // events and queue messages
        let mut events = Events::with_capacity(self.nevent);
        let mut messages = Vec::with_capacity(QUEUE_CAPACITY);
        // let mut sessions = Vec::with_capacity(QUEUE_CAPACITY);

        loop {
            // WORKER_EVENT_LOOP.increment();

            // get events with timeout
            if self.poll.poll(&mut events, Some(self.timeout)).is_err() {
                error!("Error polling");
            }

            // let timestamp = Instant::now();

            // let count = events.iter().count();
            // WORKER_EVENT_TOTAL.add(count as _);
            // if count == self.nevent {
            //     WORKER_EVENT_MAX_REACHED.increment();
            // } else {
            //     WORKER_EVENT_DEPTH.increment(timestamp, count as _, 1);
            // }

            // process all events
            for event in events.iter() {
                let token = event.token();
                match token {
                    WAKER_TOKEN => {
                        // handle all pending messages on the data queue
                        self.data_queue.try_recv_all(&mut messages);
                        for (request, fe_token) in messages.drain(..).map(|v| v.into_inner()) {
                            if let Some(be_token) = self.free_queue.pop_front() {
                                let session = &mut self.sessions[be_token.0];
                                if session.send(request).is_err() {
                                    panic!("we don't handle this right now");
                                } else {
                                    self.pending.insert(be_token, fe_token);
                                }
                            } else {
                                self.backlog.push_back((request, token));
                            }
                        }

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
                        if event.is_error() {
                            self.close(token);
                            continue;
                        }
                        if event.is_writable() && self.write(token).is_err() {
                            self.close(token);
                            continue;
                        }
                        if event.is_readable() && self.read(token).is_err() {
                            self.close(token);
                            continue;
                        }
                    }
                }
            }

            // wakes the storage thread if necessary
            let _ = self.data_queue.wake();
        }
    }
}

pub struct BackendWorkerBuilder<Parser, Request, Response> {
    free_queue: VecDeque<Token>,
    nevent: usize,
    parser: Parser,
    poll: Poll,
    sessions: Slab<ClientSession<Parser, Request, Response>>,
    timeout: Duration,
    waker: Arc<Waker>,
}

impl<Parser, Request, Response> BackendWorkerBuilder<Parser, Request, Response>
where
    Parser: Clone + Parse<Response>,
    Request: Compose,
{
    pub fn new<T: BackendConfig>(config: &T, parser: Parser) -> Result<Self> {
        let config = config.backend();

        let poll = Poll::new()?;

        let waker = Arc::new(Waker::new(poll.registry(), WAKER_TOKEN).unwrap());

        let nevent = config.nevent();
        let timeout = Duration::from_millis(config.timeout() as u64);

        let mut sessions = Slab::new();
        let mut free_queue = VecDeque::new();

        for endpoint in config.socket_addrs()? {
            let stream = TcpStream::connect(endpoint)?;
            let mut session = ClientSession::new(Session::from(stream), parser.clone());
            let s = sessions.vacant_entry();
            session
                .register(poll.registry(), Token(s.key()), session.interest())
                .expect("failed to register");
            free_queue.push_back(Token(s.key()));
            s.insert(session);
        }

        Ok(Self {
            free_queue,
            nevent,
            parser,
            poll,
            sessions,
            timeout,
            waker,
        })
    }

    pub fn waker(&self) -> Arc<Waker> {
        self.waker.clone()
    }

    pub fn build(
        self,
        data_queue: Queues<(Request, Response, Token), (Request, Token)>,
        signal_queue: Queues<(), Signal>,
    ) -> BackendWorker<Parser, Request, Response> {
        BackendWorker {
            backlog: VecDeque::new(),
            data_queue,
            free_queue: self.free_queue,
            nevent: self.nevent,
            parser: self.parser,
            pending: HashMap::new(),
            poll: self.poll,
            sessions: self.sessions,
            signal_queue,
            timeout: self.timeout,
            waker: self.waker,
        }
    }
}

pub struct BackendBuilder<Parser, Request, Response> {
    builders: Vec<BackendWorkerBuilder<Parser, Request, Response>>,
}

impl<BackendParser, BackendRequest, BackendResponse>
    BackendBuilder<BackendParser, BackendRequest, BackendResponse>
where
    BackendParser: Parse<BackendResponse> + Clone,
    // BackendResponse: Compose,
    BackendRequest: Compose,
{
    pub fn new<T: BackendConfig>(
        config: &T,
        parser: BackendParser,
        threads: usize,
    ) -> Result<Self> {
        let mut builders = Vec::new();
        for _ in 0..threads {
            builders.push(BackendWorkerBuilder::new(config, parser.clone())?);
        }
        Ok(Self { builders })
    }

    pub fn wakers(&self) -> Vec<Arc<Waker>> {
        self.builders.iter().map(|b| b.waker()).collect()
    }

    pub fn build(
        mut self,
        mut data_queues: Vec<
            Queues<(BackendRequest, BackendResponse, Token), (BackendRequest, Token)>,
        >,
        mut signal_queues: Vec<Queues<(), Signal>>,
    ) -> Vec<BackendWorker<BackendParser, BackendRequest, BackendResponse>> {
        self.builders
            .drain(..)
            .map(|b| b.build(data_queues.pop().unwrap(), signal_queues.pop().unwrap()))
            .collect()
    }
}
