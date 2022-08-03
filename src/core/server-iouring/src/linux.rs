#![allow(unused_imports)]

use buffer::*;

// use bytes::Buf;
// use bytes::BufMut;
use std::borrow::Borrow;
use std::io::Result;

use std::collections::VecDeque;
use std::net::TcpListener;
use std::os::unix::io::{AsRawFd, RawFd};
use std::{io, ptr};

use io_uring::{opcode, squeue, types, IoUring, SubmissionQueue};
use slab::Slab;

use std::sync::mpsc::*;

use ::net::TcpStream;
use std::os::unix::io::FromRawFd;

use session_common::ServerSession;

use protocol_ping::*;

// mod buffer;
// use buffer::Buffer;

const TIMEOUT_TOKEN: u64 = u64::MAX - 1;
const LISTENER_TOKEN: u64 = u64::MAX;

#[derive(Clone, Copy, Debug)]
enum State {
    Poll,
    Read,
    Write,
    Shutdown,
}

pub struct Session {
    inner: ServerSession<RequestParser, Response, Request>,
    state: State,
    fd: RawFd,
}

pub struct Worker {
    backlog: VecDeque<squeue::Entry>,
    ring: IoUring,
    sessions: Slab<Session>,
    session_tx: Sender<Session>,
    session_rx: Receiver<Session>,
}

impl Worker {
    pub fn new(session_tx: Sender<Session>, session_rx: Receiver<Session>) -> Result<Self> {
        let ring = IoUring::builder().build(8192)?;
        let sessions = Slab::new();
        let backlog = VecDeque::new();

        Ok(Self {
            backlog,
            ring,
            sessions,
            session_tx,
            session_rx,
        })
    }

    pub fn close(&mut self, token: usize) {
        let session = self.sessions.remove(token);
        let fd = session.fd;
        let _ = self.session_tx.send(session);
    }

    pub fn read(&mut self, token: usize) {
        let session = &mut self.sessions[token];

        if let Ok(request) = session.inner.receive() {
            let send = match request {
                Request::Ping => session.inner.send(Response::Pong),
            };

            if send.is_ok() {
                session.state = State::Write;

                let entry = opcode::Send::new(
                    types::Fd(session.fd),
                    session.inner.write_buffer_mut().read_ptr(),
                    session.inner.write_buffer_mut().remaining() as _,
                )
                .build()
                .user_data(token as _);

                unsafe {
                    if self.ring.submission().push(&entry).is_err() {
                        self.backlog.push_back(entry);
                    }
                }
            } else {
                let session = self.sessions.remove(token);
                let _ = self.session_tx.send(session);
            }
        } else {
            let session = self.sessions.remove(token);
            let _ = self.session_tx.send(session);
        }
    }

    /// Puts a read operation onto the submission queue. If the submission queue
    /// is full, it will be placed on the backlog instead.
    pub fn submit_read(&mut self, token: usize) {
        let session = &mut self.sessions[token];

        session.state = State::Read;

        let entry = opcode::Read::new(
            types::Fd(session.fd),
            session.inner.read_buffer_mut().write_ptr(),
            session.inner.read_buffer_mut().remaining_mut() as _,
        )
        .build()
        .user_data(token as _);

        unsafe {
            if self.ring.submission().push(&entry).is_err() {
                self.backlog.push_back(entry);
            }
        }
    }

    /// Puts a write operation onto the submission queue. If the submission
    /// queue is full, it will be placed on the backlog instead.
    pub fn submit_write(&mut self, token: usize) {
        let session = &mut self.sessions[token];

        session.state = State::Write;

        let entry = opcode::Write::new(
            types::Fd(session.fd),
            session.inner.write_buffer_mut().read_ptr(),
            session.inner.write_buffer_mut().remaining() as _,
        )
        .build()
        .user_data(token as _);

        unsafe {
            if self.ring.submission().push(&entry).is_err() {
                self.backlog.push_back(entry);
            }
        }
    }

    pub fn run(mut self) {
        // let (submitter, mut sq, mut cq) = self.ring.split();

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

            // take one session from the queue, add it to the sessions slab, and
            // submit a read to the kernel
            if let Ok(session) = self.session_rx.try_recv() {
                let token = self.sessions.insert(session);

                self.submit_read(token);
            }

            // to avoid borrow issues, we write this as a while loop instead of
            // a for loop
            let mut next = self.ring.completion().next();

            while let Some(cqe) = next.take() {
                let ret = cqe.result();
                let token = cqe.user_data() as usize;

                // timeouts get resubmitted
                if token == TIMEOUT_TOKEN as _ {
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
                    eprintln!(
                        "token {:?} error: {:?}",
                        self.sessions.get(token).map(|v| v.state.clone()),
                        io::Error::from_raw_os_error(-ret)
                    );
                    continue;
                }

                let session = &mut self.sessions[token];

                match session.state {
                    State::Read => {
                        if ret == 0 {
                            self.close(token);
                        } else {
                            // mark the read buffer as containing the number of
                            // bytes read into it by the kernel
                            unsafe {
                                session.inner.read_buffer_mut().advance_mut(ret as usize);
                            }

                            self.read(token);
                        }
                    }
                    State::Write => {
                        // advance the write buffer by the number of bytes that
                        // were written to the underlying stream
                        session.inner.write_buffer_mut().advance(ret as usize);

                        // if the write buffer is now empty, we want to resume
                        // reading, otherwise submit a write so we can finish
                        // flushing the buffer.
                        if session.inner.write_buffer_mut().remaining() == 0 {
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
        }
    }
}

pub struct Listener {
    // acceptor: Acceptor,
    backlog: VecDeque<squeue::Entry>,
    ring: IoUring,
    #[allow(dead_code)]
    listener: TcpListener,
    sessions: Slab<Session>,
    session_tx: Sender<Session>,
    session_rx: Receiver<Session>,
    accept_backlog: usize,
}

impl Listener {
    pub fn new(session_tx: Sender<Session>, session_rx: Receiver<Session>) -> Result<Self> {
        let ring = IoUring::builder().build(8192)?;
        let listener = TcpListener::bind("127.0.0.1:12321")?;
        let backlog = VecDeque::new();
        let sessions = Slab::new();

        Ok(Self {
            backlog,
            ring,
            listener,
            sessions,
            session_tx,
            session_rx,
            accept_backlog: 1024,
        })
    }

    pub fn submit_shutdown(&mut self, token: usize) {
        let session = &mut self.sessions[token];

        session.state = State::Shutdown;

        let entry = opcode::Shutdown::new(types::Fd(session.fd), libc::SHUT_RDWR)
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

        session.state = State::Poll;

        let event = opcode::PollAdd::new(types::Fd(session.fd), libc::POLLIN as _)
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
            if let Ok(session) = self.session_rx.try_recv() {
                let token = self.sessions.insert(session);
                self.submit_shutdown(token);
            }

            // to prevent borrow issues, this is implemented as a while loop
            // instead of a for loop
            let mut next = self.ring.completion().next();

            while let Some(cqe) = next.take() {
                let ret = cqe.result();
                let token = cqe.user_data();

                // replace timeout token with a new one and move on to other
                // completions
                if token == TIMEOUT_TOKEN as _ {
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
                                Ok(_) => self.accept_backlog -= 1,
                                Err(_) => break,
                            }
                        }

                        // create a session and submit a poll for it
                        let tcp_stream = unsafe { TcpStream::from_raw_fd(ret) };
                        let session = ServerSession::new(
                            session_common::Session::from(tcp_stream),
                            RequestParser::new(),
                        );
                        let session = Session {
                            inner: session,
                            state: State::Poll,
                            fd: ret,
                        };

                        let token = self.sessions.insert(session);
                        self.submit_poll(token);
                    }
                    token => {
                        let token = token as usize;
                        let session = &self.sessions[token];
                        match session.state {
                            State::Poll => {
                                let session = self.sessions.remove(token);
                                let _ = self.session_tx.send(session);
                            }
                            State::Shutdown => {
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
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
