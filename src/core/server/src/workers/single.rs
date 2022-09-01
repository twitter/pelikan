// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use super::*;
use std::collections::VecDeque;

pub struct SingleWorkerBuilder<Parser, Request, Response, Storage> {
    nevent: usize,
    parser: Parser,
    pending: VecDeque<Token>,
    poll: Poll,
    sessions: Slab<ServerSession<Parser, Response, Request>>,
    storage: Storage,
    timeout: Duration,
    waker: Arc<Box<dyn waker::Waker>>,
}

impl<Parser, Request, Response, Storage> SingleWorkerBuilder<Parser, Request, Response, Storage> {
    pub fn new<T: WorkerConfig>(config: &T, parser: Parser, storage: Storage) -> Result<Self> {
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
            pending: VecDeque::new(),
            poll,
            sessions: Slab::new(),
            storage,
            timeout,
            waker,
        })
    }

    pub fn waker(&self) -> Arc<Box<dyn waker::Waker>> {
        self.waker.clone()
    }

    pub fn build(
        self,
        session_queue: Queues<Session, Session>,
        signal_queue: Queues<(), Signal>,
    ) -> SingleWorker<Parser, Request, Response, Storage> {
        SingleWorker {
            nevent: self.nevent,
            parser: self.parser,
            pending: self.pending,
            poll: self.poll,
            session_queue,
            sessions: self.sessions,
            signal_queue,
            storage: self.storage,
            timeout: self.timeout,
            waker: self.waker,
        }
    }
}

pub struct SingleWorker<Parser, Request, Response, Storage> {
    nevent: usize,
    parser: Parser,
    pending: VecDeque<Token>,
    poll: Poll,
    session_queue: Queues<Session, Session>,
    sessions: Slab<ServerSession<Parser, Response, Request>>,
    signal_queue: Queues<(), Signal>,
    storage: Storage,
    timeout: Duration,
    waker: Arc<Box<dyn waker::Waker>>,
}

impl<Parser, Request, Response, Storage> SingleWorker<Parser, Request, Response, Storage>
where
    Parser: Parse<Request> + Clone,
    Request: Klog + Klog<Response = Response>,
    Response: Compose,
    Storage: EntryStore + Execute<Request, Response>,
{
    /// Return the `Session` to the `Listener` to handle flush/close
    fn close(&mut self, token: Token) {
        if self.sessions.contains(token.0) {
            let mut session = self.sessions.remove(token.0).into_inner();
            let _ = self.poll.registry().deregister(&mut session);
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

        // process up to one pending request
        match session.receive() {
            Ok(request) => {
                let response = self.storage.execute(&request);
                PROCESS_REQ.increment();
                if response.should_hangup() {
                    let _ = session.send(response);
                    return Err(Error::new(ErrorKind::Other, "should hangup"));
                }
                request.klog(&response);
                match session.send(response) {
                    Ok(_) => {
                        // attempt to flush immediately if there's now data in
                        // the write buffer
                        if session.write_pending() > 0 {
                            match session.flush() {
                                Ok(_) => Ok(()),
                                Err(e) => map_err(e),
                            }?;
                        }

                        // reregister to get writable event
                        if session.write_pending() > 0 {
                            let interest = session.interest();
                            if self
                                .poll
                                .registry()
                                .reregister(session, token, interest)
                                .is_err()
                            {
                                return Err(Error::new(ErrorKind::Other, "failed to reregister"));
                            }
                        }

                        // if there's still data to read, put the token on the
                        // pending queue
                        if session.remaining() > 0 {
                            self.pending.push_back(token);
                        }

                        Ok(())
                    }
                    Err(e) => {
                        if e.kind() == ErrorKind::WouldBlock {
                            Ok(())
                        } else {
                            Err(e)
                        }
                    }
                }
            }
            Err(e) => {
                if e.kind() == ErrorKind::WouldBlock {
                    Ok(())
                } else {
                    Err(e)
                }
            }
        }
    }

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
        let mut events = Events::with_capacity(self.nevent);

        loop {
            WORKER_EVENT_LOOP.increment();

            self.storage.expire();

            // we need another wakeup if there are still pending reads
            if !self.pending.is_empty() {
                let _ = self.waker.wake();
            }

            // get events with timeout
            if self.poll.poll(&mut events, Some(self.timeout)).is_err() {
                error!("Error polling");
            }

            let timestamp = Instant::now();

            let count = events.iter().count();
            WORKER_EVENT_TOTAL.add(count as _);
            if count == self.nevent {
                WORKER_EVENT_MAX_REACHED.increment();
            } else {
                WORKER_EVENT_DEPTH.increment(timestamp, count as _, 1);
            }

            // process all events
            for event in events.iter() {
                let token = event.token();

                match token {
                    WAKER_TOKEN => {
                        // handle outstanding reads
                        for _ in 0..self.pending.len() {
                            if let Some(token) = self.pending.pop_front() {
                                if self.read(token).is_err() {
                                    self.close(token);
                                }
                            }
                        }

                        // handle up to one new session
                        if let Some(mut session) =
                            self.session_queue.try_recv().map(|v| v.into_inner())
                        {
                            let s = self.sessions.vacant_entry();
                            let interest = session.interest();
                            if session
                                .register(self.poll.registry(), Token(s.key()), interest)
                                .is_ok()
                            {
                                s.insert(ServerSession::new(session, self.parser.clone()));
                            } else {
                                let _ = self.session_queue.try_send_any(session);
                            }

                            // trigger a wake-up in case there are more sessions
                            let _ = self.waker.wake();
                        }

                        // check if we received any signals from the admin thread
                        while let Some(signal) = self.signal_queue.try_recv() {
                            match signal.into_inner() {
                                Signal::FlushAll => {
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
                        if event.is_error() {
                            WORKER_EVENT_ERROR.increment();

                            self.close(token);
                            continue;
                        }

                        if event.is_writable() {
                            WORKER_EVENT_WRITE.increment();

                            if self.write(token).is_err() {
                                self.close(token);
                                continue;
                            }
                        }

                        if event.is_readable() {
                            WORKER_EVENT_READ.increment();

                            if self.read(token).is_err() {
                                self.close(token);
                                continue;
                            }
                        }
                    }
                }
            }
        }
    }
}
