// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use config::*;
use io_uring::{opcode, squeue, types, IoUring};
use net::TcpStream;
use protocol_common::*;
use rustcommon_metrics::*;
use session_common::ServerSession;
use slab::Slab;

use std::collections::VecDeque;
use std::io::Result;
use std::marker::PhantomData;
use std::net::TcpListener;
use std::os::unix::io::AsRawFd;
use std::os::unix::io::FromRawFd;
use std::sync::Arc;
use std::{io, ptr};

use super::*;

counter!(LISTENER_EVENT_ERROR);
counter!(LISTENER_EVENT_LOOP);
counter!(LISTENER_EVENT_TOTAL);

counter!(LISTENER_SESSION_DROP);
counter!(LISTENER_SESSION_SHUTDOWN);

pub struct ListenerBuilder<Parser, Request, Response>
where
    Parser: Parse<Request> + Clone + Send,
    Request: Send,
    Response: Compose + Send,
{
    backlog: VecDeque<squeue::Entry>,
    listener: TcpListener,
    parser: Parser,
    ring: IoUring,
    sessions: Slab<Session<Parser, Request, Response>>,
    waker: Arc<Box<dyn Waker>>,
    _request: PhantomData<Request>,
    _response: PhantomData<Response>,
}

impl<Parser, Request, Response> ListenerBuilder<Parser, Request, Response>
where
    Parser: Parse<Request> + Clone + Send,
    Request: Send,
    Response: Compose + Send,
{
    pub fn new<T: ServerConfig + TlsConfig>(config: &T, parser: Parser) -> Result<Self> {
        let config = config.server();

        let addr = config.socket_addr().map_err(|e| {
            error!("{}", e);
            std::io::Error::new(std::io::ErrorKind::Other, "Bad listen address")
        })?;

        let ring = IoUring::builder().build(8192)?;
        let listener = TcpListener::bind(addr)?;
        let sessions = Slab::<Session<Parser, Request, Response>>::new();
        let backlog = VecDeque::new();
        let waker = Arc::new(Box::new(EventfdWaker::new()?) as Box<dyn Waker>);

        Ok(Self {
            backlog,
            listener,
            parser,
            ring,
            sessions,
            waker,
            _request: PhantomData,
            _response: PhantomData,
        })
    }

    pub fn build(
        self,
        session_queue: Queues<
            Session<Parser, Request, Response>,
            Session<Parser, Request, Response>,
        >,
    ) -> Listener<Parser, Request, Response> {
        Listener {
            accept_backlog: 1024,
            backlog: self.backlog,
            listener: self.listener,
            parser: self.parser,
            ring: self.ring,
            sessions: self.sessions,
            session_queue,
            waker: self.waker,
            _request: PhantomData,
            _response: PhantomData,
        }
    }

    pub fn waker(&self) -> Arc<Box<dyn Waker>> {
        self.waker.clone()
    }
}

pub struct Listener<Parser, Request, Response>
where
    Parser: Parse<Request> + Send,
    Request: Send,
    Response: Compose + Send,
{
    accept_backlog: usize,
    backlog: VecDeque<squeue::Entry>,
    ring: IoUring,
    #[allow(dead_code)]
    listener: TcpListener,
    parser: Parser,
    sessions: Slab<Session<Parser, Request, Response>>,
    session_queue: Queues<Session<Parser, Request, Response>, Session<Parser, Request, Response>>,
    waker: Arc<Box<dyn Waker>>,
    _request: PhantomData<Request>,
    _response: PhantomData<Response>,
}

impl<Parser, Request, Response> Listener<Parser, Request, Response>
where
    Parser: Parse<Request> + Clone + Send,
    Request: Send,
    Response: Compose + Send,
{
    pub fn submit_shutdown(&mut self, token: usize) {
        let session = &mut self.sessions[token];

        session.set_state(State::Shutdown);

        let entry = opcode::Shutdown::new(types::Fd(session.as_raw_fd()), libc::SHUT_RDWR)
            .build()
            .user_data(token as _);

        unsafe {
            if self.ring.submission().push(&entry).is_err() {
                self.backlog.push_back(entry);
            }
        }
    }

    pub fn submit_poll(&mut self, token: usize) {
        let session = &mut self.sessions[token];

        session.set_state(State::Poll);

        let event = opcode::PollAdd::new(types::Fd(session.as_raw_fd()), libc::POLLIN as _)
            .build()
            .user_data(token as _);

        unsafe {
            if self.ring.submission().push(&event).is_err() {
                self.backlog.push_back(event);
            }
        }
    }

    pub fn run(mut self) {
        // let (submitter, mut sq, mut cq) = self.ring.split();

        for _ in 0..1024 {
            let entry = opcode::Accept::new(
                types::Fd(self.listener.as_raw_fd()),
                ptr::null_mut(),
                ptr::null_mut(),
            )
            .build()
            .user_data(LISTENER_TOKEN);

            unsafe {
                match self.ring.submission().push(&entry) {
                    Ok(_) => self.accept_backlog -= 1,
                    Err(_) => break,
                }
            }
        }

        let timeout_ts = types::Timespec::new().nsec(1_000_000);

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
            LISTENER_EVENT_LOOP.increment();

            match self.ring.submit_and_wait(1) {
                Ok(_) => (),
                Err(ref err) if err.raw_os_error() == Some(libc::EBUSY) => (),
                Err(err) => panic!("{}", err),
            }

            self.ring.completion().sync();

            // handle backlog
            loop {
                if self.ring.submission().is_full() {
                    match self.ring.submit() {
                        Ok(_) => (),
                        Err(ref e) if e.raw_os_error() == Some(libc::EBUSY) => break,
                        Err(e) => panic!("{}", e),
                    }
                }

                self.ring.submission().sync();

                match self.backlog.pop_front() {
                    Some(sqe) => unsafe {
                        let _ = self.ring.submission().push(&sqe);
                    },
                    None => break,
                }
            }

            // if there are any sessions to shutdown, take one and submit a
            // shutdown for it
            if let Some(session) = self.session_queue.try_recv().map(|v| v.into_inner()) {
                let token = self.sessions.insert(session);
                self.submit_shutdown(token);
            }

            // to prevent borrow issues, this is implemented as a while loop
            // instead of a for loop
            let mut next = self.ring.completion().next();

            while let Some(cqe) = next.take() {
                LISTENER_EVENT_TOTAL.increment();

                let ret = cqe.result();
                let token = cqe.user_data();

                // replace timeout token with a new one and move on to other
                // completions
                if token == TIMEOUT_TOKEN as u64 {
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

                // handle error result here
                if ret < 0 {
                    LISTENER_EVENT_ERROR.increment();

                    eprintln!("error: {:?}", io::Error::from_raw_os_error(-ret));
                    continue;
                }

                match token {
                    LISTENER_TOKEN => {
                        // add another accept to the submission queue to replace
                        // this one
                        let entry = opcode::Accept::new(
                            types::Fd(self.listener.as_raw_fd()),
                            ptr::null_mut(),
                            ptr::null_mut(),
                        )
                        .build()
                        .user_data(LISTENER_TOKEN);

                        unsafe {
                            match self.ring.submission().push(&entry) {
                                Ok(_) => {
                                    self.accept_backlog = self.accept_backlog.saturating_sub(1)
                                }
                                Err(_) => break,
                            }
                        }

                        // create a session and submit a poll for it
                        let tcp_stream = unsafe { TcpStream::from_raw_fd(ret) };
                        let session = ServerSession::new(
                            session_common::Session::from(tcp_stream),
                            self.parser.clone(),
                        );
                        let session = Session::from(session);
                        let token = self.sessions.insert(session);
                        self.submit_poll(token);
                    }
                    token => {
                        let token = token as usize;
                        let session = &self.sessions[token];
                        match session.state() {
                            State::Poll => {
                                let session = self.sessions.remove(token);
                                if self.session_queue.try_send_any(session).is_err() {
                                    LISTENER_SESSION_DROP.increment();
                                }
                            }
                            State::Shutdown => {
                                LISTENER_SESSION_SHUTDOWN.increment();
                                let _ = self.sessions.remove(token);
                            }
                            _ => {
                                panic!("unexpected session state");
                            }
                        }
                    }
                }

                next = self.ring.completion().next();
            }

            self.session_queue.wake();
        }
    }
}
