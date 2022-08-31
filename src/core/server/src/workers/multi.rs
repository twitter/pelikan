// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use super::map_result;
use crate::*;

pub struct MultiWorkerBuilder<Parser, Request, Response> {
    nevent: usize,
    parser: Parser,
    poll: Poll,
    sessions: Slab<ServerSession<Parser, Response, Request>>,
    timeout: Duration,
    waker: Arc<Box<dyn waker::Waker>>,
}

impl<Parser, Request, Response> MultiWorkerBuilder<Parser, Request, Response> {
    pub fn new<T: WorkerConfig>(config: &T, parser: Parser) -> Result<Self> {
        let config = config.worker();

        let poll = Poll::new()?;

        let waker =
            Arc::new(Box::new(Waker::new(poll.registry(), WAKER_TOKEN).unwrap())
                as Box<dyn waker::Waker>);

        let nevent = config.nevent();
        let timeout = Duration::from_millis(config.timeout() as u64);

        Ok(Self {
            nevent,
            parser,
            poll,
            sessions: Slab::new(),
            timeout,
            waker,
        })
    }

    pub fn waker(&self) -> Arc<Box<dyn waker::Waker>> {
        self.waker.clone()
    }

    pub fn build(
        self,
        data_queue: Queues<(Request, Token), (Request, Response, Token)>,
        session_queue: Queues<Session, Session>,
        signal_queue: Queues<(), Signal>,
    ) -> MultiWorker<Parser, Request, Response> {
        MultiWorker {
            data_queue,
            nevent: self.nevent,
            parser: self.parser,
            poll: self.poll,
            session_queue,
            sessions: self.sessions,
            signal_queue,
            timeout: self.timeout,
            waker: self.waker,
        }
    }
}

pub struct MultiWorker<Parser, Request, Response> {
    data_queue: Queues<(Request, Token), (Request, Response, Token)>,
    nevent: usize,
    parser: Parser,
    poll: Poll,
    session_queue: Queues<Session, Session>,
    sessions: Slab<ServerSession<Parser, Response, Request>>,
    signal_queue: Queues<(), Signal>,
    timeout: Duration,
    waker: Arc<Box<dyn waker::Waker>>,
}

impl<Parser, Request, Response> MultiWorker<Parser, Request, Response>
where
    Parser: Parse<Request> + Clone,
    Request: Klog + Klog<Response = Response>,
    Response: Compose,
{
    /// Return the `Session` to the `Listener` to handle flush/close
    fn close(&mut self, token: Token) {
        if self.sessions.contains(token.0) {
            let mut session = self.sessions.remove(token.0).into_inner();
            let _ = session.deregister(self.poll.registry());
            let _ = self.session_queue.try_send_any(session);
            let _ = self.session_queue.wake();
        }
    }

    /// Handle up to one request for a session
    fn read(&mut self, token: Token) -> Result<()> {
        let session = self
            .sessions
            .get_mut(token.0)
            .ok_or_else(|| Error::new(ErrorKind::Other, "non-existant session"))?;

        // fill the session
        map_result(session.fill())?;

        // process up to one request
        match session.receive() {
            Ok(request) => self
                .data_queue
                .try_send_to(0, (request, token))
                .map_err(|_| Error::new(ErrorKind::Other, "data queue is full")),
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
                        // handle up to one new session
                        if let Some(mut session) =
                            self.session_queue.try_recv().map(|v| v.into_inner())
                        {
                            let s = self.sessions.vacant_entry();
                            if session
                                .register(self.poll.registry(), Token(s.key()), session.interest())
                                .is_ok()
                            {
                                s.insert(ServerSession::new(session, self.parser.clone()));
                            } else {
                                let _ = self.session_queue.try_send_any(session);
                            }

                            // trigger a wake-up in case there are more sessions
                            let _ = self.waker.wake();
                        }

                        // handle all pending messages on the data queue
                        self.data_queue.try_recv_all(&mut messages);
                        for (request, response, token) in
                            messages.drain(..).map(|v| v.into_inner())
                        {
                            request.klog(&response);
                            if let Some(session) = self.sessions.get_mut(token.0) {
                                if response.should_hangup() {
                                    let _ = session.send(response);
                                    self.close(token);
                                    continue;
                                } else if session.send(response).is_err()
                                    || (session.write_pending() > 0
                                        && session
                                            .reregister(
                                                self.poll.registry(),
                                                token,
                                                session.interest(),
                                            )
                                            .is_err())
                                {
                                    self.close(token);
                                    continue;
                                }
                                if session.remaining() > 0 && self.read(token).is_err() {
                                    self.close(token);
                                    continue;
                                }
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
