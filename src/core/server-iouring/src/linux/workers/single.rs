// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use entrystore::EntryStore;
use io_uring::{opcode, squeue, types, IoUring};
use protocol_common::*;
use rustcommon_metrics::*;
use slab::Slab;

use std::collections::VecDeque;
use std::io;
use std::io::{ErrorKind, Result};
use std::marker::PhantomData;
use std::sync::Arc;

use super::*;

counter!(WORKER_EVENT_ERROR);
counter!(WORKER_EVENT_LOOP);
counter!(WORKER_EVENT_TOTAL);

counter!(WORKER_SESSION_CLOSE);
counter!(WORKER_SESSION_READ);
counter!(WORKER_SESSION_WRITE);

counter!(WORKER_SUBMIT_READ);
counter!(WORKER_SUBMIT_WRITE);

counter!(WORKER_BACKLOG_PUSH);
counter!(WORKER_BACKLOG_POP);

// counter!(LISTENER_SESSION_DROP);
// counter!(LISTENER_SESSION_SHUTDOWN);

pub struct SingleWorkerBuilder<Parser, Request, Response, Storage>
where
    Parser: Parse<Request> + Send,
    Request: Send,
    Response: Compose + Send,
    Storage: EntryStore + Execute<Request, Response>,
{
    backlog: VecDeque<squeue::Entry>,
    parser: Parser,
    ring: IoUring,
    sessions: Slab<Session<Parser, Request, Response>>,
    storage: Storage,
    waker: Arc<Box<dyn Waker>>,
    _parser: PhantomData<Parser>,
    _request: PhantomData<Request>,
    _response: PhantomData<Response>,
    _storage: PhantomData<Storage>,
}

impl<Parser, Request, Response, Storage> SingleWorkerBuilder<Parser, Request, Response, Storage>
where
    Parser: Parse<Request> + Send,
    Request: Send,
    Response: Compose + Send,
    Storage: EntryStore + Execute<Request, Response>,
{
    pub fn new(parser: Parser, storage: Storage) -> Result<Self> {
        let ring = IoUring::builder().build(16384)?;
        let sessions = Slab::<Session<Parser, Request, Response>>::new();
        let backlog = VecDeque::new();
        let waker = Arc::new(Box::new(EventfdWaker::new()?) as Box<dyn Waker>);

        Ok(Self {
            backlog,
            parser,
            ring,
            sessions,
            storage,
            waker,
            _parser: PhantomData,
            _request: PhantomData,
            _response: PhantomData,
            _storage: PhantomData,
        })
    }

    pub fn build(
        self,
        session_queue: Queues<
            Session<Parser, Request, Response>,
            Session<Parser, Request, Response>,
        >,
    ) -> SingleWorker<Parser, Request, Response, Storage> {
        SingleWorker {
            backlog: self.backlog,
            parser: self.parser,
            ring: self.ring,
            sessions: self.sessions,
            session_queue,
            storage: self.storage,
            waker: self.waker,
            _request: PhantomData,
            _response: PhantomData,
        }
    }

    pub fn waker(&self) -> Arc<Box<dyn Waker>> {
        self.waker.clone()
    }
}

pub struct SingleWorker<Parser, Request, Response, Storage>
where
    Parser: Parse<Request> + Send,
    Request: Send,
    Response: Compose + Send,
    Storage: EntryStore + Execute<Request, Response>,
{
    backlog: VecDeque<squeue::Entry>,
    parser: Parser,
    ring: IoUring,
    sessions: Slab<Session<Parser, Request, Response>>,
    session_queue: Queues<Session<Parser, Request, Response>, Session<Parser, Request, Response>>,
    storage: Storage,
    waker: Arc<Box<dyn Waker>>,
    _request: PhantomData<Request>,
    _response: PhantomData<Response>,
}

impl<Parser, Request, Response, Storage> SingleWorker<Parser, Request, Response, Storage>
where
    Parser: Parse<Request> + Send,
    Request: Send,
    Response: Compose + Send,
    Storage: EntryStore + Execute<Request, Response>,
{
    pub fn close(&mut self, token: u64) {
        let session = self.sessions.remove(token as usize);
        let _ = self.session_queue.try_send_any(session);
    }

    pub fn read(&mut self, token: u64) {
        let session = &mut self.sessions[token as usize];

        match session.receive() {
            Ok(request) => {
                let response = self.storage.execute(&request);

                let send = session.send(response);

                if send.is_ok() {
                    session.set_state(State::Write);
                    self.submit_write(token);
                } else {
                    WORKER_SESSION_CLOSE.increment();
                    info!("failed to send, removing session: {}", token);
                    self.close(token);
                }
            }
            Err(e) => {
                if e.kind() == ErrorKind::WouldBlock {
                    info!("wouldblock for session: {}", token);
                    assert!(session.read_buffer_mut().remaining_mut() > 0);
                    self.submit_read(token);
                } else {
                    WORKER_SESSION_CLOSE.increment();
                    info!("bad request, removing session: {}", token);
                    self.close(token);
                }
            }
        }
    }

    /// Puts a read operation onto the submission queue. If the submission queue
    /// is full, it will be placed on the backlog instead.
    pub fn submit_read(&mut self, token: u64) {
        WORKER_SUBMIT_READ.increment();

        let session = &mut self.sessions[token as usize];

        session.set_state(State::Read);

        assert!(session.read_buffer_mut().remaining_mut() > 0);

        let entry = opcode::Recv::new(
            types::Fd(session.as_raw_fd()),
            session.read_buffer_mut().write_ptr(),
            session.read_buffer_mut().remaining_mut() as _,
        )
        .build()
        .user_data(token as _);

        unsafe {
            if self.ring.submission().push(&entry).is_err() {
                WORKER_BACKLOG_PUSH.increment();
                self.backlog.push_back(entry);
            }
        }
    }

    /// Puts a write operation onto the submission queue. If the submission
    /// queue is full, it will be placed on the backlog instead.
    pub fn submit_write(&mut self, token: u64) {
        WORKER_SUBMIT_WRITE.increment();

        let session = &mut self.sessions[token as usize];

        session.set_state(State::Write);

        let entry = opcode::Send::new(
            types::Fd(session.as_raw_fd()),
            session.write_buffer_mut().read_ptr(),
            session.write_buffer_mut().remaining() as _,
        )
        .build()
        .user_data(token as _);

        unsafe {
            if self.ring.submission().push(&entry).is_err() {
                WORKER_BACKLOG_PUSH.increment();
                self.backlog.push_back(entry);
            }
        }
    }

    pub fn run(mut self) {
        // let (submitter, mut sq, mut cq) = self.ring.split();

        let timeout_ts = types::Timespec::new().nsec(100_000_000);

        let timeout = opcode::Timeout::new(&timeout_ts)
            .build()
            .user_data(TIMEOUT_TOKEN as _);
        unsafe {
            match self.ring.submission().push(&timeout) {
                Ok(_) => {}
                Err(_) => {
                    panic!("failed to register timeout");
                }
            }
        }

        self.ring.submission().sync();

        loop {
            match self.ring.submit_and_wait(1) {
                Ok(_) => (),
                Err(ref err) if err.raw_os_error() == Some(libc::EBUSY) => (),
                Err(err) => panic!("{}", err),
            }

            self.ring.completion().sync();

            // handle backlog
            loop {
                // info!("loop");
                if self.ring.submission().is_full() {
                    info!("submission queue is full");
                    match self.ring.submit() {
                        Ok(_) => (),
                        Err(ref e) if e.raw_os_error() == Some(libc::EBUSY) => break,
                        Err(e) => panic!("{}", e),
                    }
                }

                self.ring.submission().sync();

                match self.backlog.pop_front() {
                    Some(sqe) => unsafe {
                        WORKER_BACKLOG_POP.increment();
                        info!("adding backlog event to submission queue");
                        let _ = self.ring.submission().push(&sqe);
                    },
                    None => break,
                }
            }

            // take one session from the queue, add it to the sessions slab, and
            // submit a read to the kernel
            if let Some(session) = self.session_queue.try_recv().map(|v| v.into_inner()) {
                let token = self.sessions.insert(session);

                self.submit_read(token as u64);
            }

            // to avoid borrow issues, we write this as a while loop instead of
            // a for loop
            let mut next = self.ring.completion().next();

            let mut count = 0;

            while let Some(cqe) = next.take() {
                WORKER_EVENT_TOTAL.increment();

                count += 1;

                let ret = cqe.result();
                let token = cqe.user_data();

                // timeouts get resubmitted
                if token == TIMEOUT_TOKEN {
                    trace!("re-add timeout event");
                    count = 0;
                    unsafe {
                        match self.ring.submission().push(&timeout) {
                            Ok(_) => {}
                            Err(_) => {
                                panic!("failed to register timeout");
                            }
                        }
                    }
                    continue;
                }

                // handle any errors here
                if ret < 0 {
                    WORKER_EVENT_ERROR.increment();

                    eprintln!(
                        "token {:?} error: {:?}",
                        self.sessions.get(token as usize).map(|v| v.state().clone()),
                        io::Error::from_raw_os_error(-ret)
                    );
                    continue;
                }

                let session = &mut self.sessions[token as usize];

                match session.state() {
                    State::Read => {
                        if ret == 0 {
                            WORKER_SESSION_CLOSE.increment();
                            info!("session is closed: {}", token);
                            info!(
                                "session has pending bytes: {}",
                                session.read_buffer_mut().remaining()
                            );
                            info!(
                                "session has remaining bytes: {}",
                                session.read_buffer_mut().remaining_mut()
                            );
                            self.close(token);
                        } else {
                            WORKER_SESSION_READ.increment();
                            // mark the read buffer as containing the number of
                            // bytes read into it by the kernel
                            unsafe {
                                session.read_buffer_mut().advance_mut(ret as usize);
                            }

                            self.read(token);
                        }
                    }
                    State::Write => {
                        WORKER_SESSION_WRITE.increment();
                        // advance the write buffer by the number of bytes that
                        // were written to the underlying stream
                        session.write_buffer_mut().advance(ret as usize);

                        // if the write buffer is now empty, we want to resume
                        // reading, otherwise submit a write so we can finish
                        // flushing the buffer.
                        if session.write_buffer_mut().remaining() == 0 {
                            self.submit_read(token);
                        } else {
                            self.submit_write(token);
                        };
                    }
                    _ => {
                        // this shouldn't happen here
                        panic!("unexpected session state");
                    }
                }

                next = self.ring.completion().next();
            }

            self.session_queue.wake();
        }
    }
}
