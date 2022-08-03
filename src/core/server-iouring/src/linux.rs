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

// mod buffer;
// use buffer::Buffer;

const LISTENER_TOKEN: u64 = u64::MAX;

#[derive(Clone, Copy, Debug)]
enum State {
    Poll,
    Read,
    Write,
    Shutdown,
}

pub struct Session {
    read_buffer: Buffer,
    write_buffer: Buffer,
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

    pub fn run(mut self) {
        let (submitter, mut sq, mut cq) = self.ring.split();

        let timeout_ts = types::Timespec::new().nsec(1_000_000);

        let timeout = opcode::Timeout::new(&timeout_ts).build().user_data(LISTENER_TOKEN as _);
        unsafe {
            match sq.push(&timeout) {
                Ok(_) => {},
                Err(_) => {
                    panic!("failed to register timeout");
                },
            }
        }

        sq.sync();


        loop {
            match submitter.submit_and_wait(1) {
                Ok(_) => (),
                Err(ref err) if err.raw_os_error() == Some(libc::EBUSY) => (),
                Err(err) => panic!("{}", err),
            }

            cq.sync();

            // handle backlog
            loop {
                if sq.is_full() {
                    match submitter.submit() {
                        Ok(_) => (),
                        Err(ref e) if e.raw_os_error() == Some(libc::EBUSY) => break,
                        Err(e) => panic!("{}", e),
                    }
                }

                sq.sync();

                match self.backlog.pop_front() {
                    Some(sqe) => unsafe {
                        let _ = sq.push(&sqe);
                    },
                    None => break,
                }
            }

            if let Ok(mut session) = self.session_rx.try_recv() {
                let fd = session.fd;
                let ptr = session.read_buffer.write_ptr();
                let len = session.read_buffer.remaining_mut() as u32;

                session.state = State::Read;
                let token = self.sessions.insert(session);

                let entry = opcode::Read::new(types::Fd(fd), ptr, len)
                                .build()
                                .user_data(token as _);

                unsafe {
                    if sq.push(&entry).is_err() {
                        self.backlog.push_back(entry);
                    }
                }
            }

            for cqe in &mut cq {
                let ret = cqe.result();
                let token = cqe.user_data() as usize;

                if token == LISTENER_TOKEN as _ {
                    unsafe {
                        match sq.push(&timeout) {
                            Ok(_) => {},
                            Err(_) => {
                                panic!("failed to register timeout");
                            },
                        }
                    }
                    continue;
                }

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
                            let session = self.sessions.remove(token);
                            let fd = session.fd;
                            if self.session_tx.send(session).is_err() {
                                unsafe {
                                    libc::close(fd);
                                }
                            }
                        } else {
                            let len = ret as usize;

                            unsafe {
                                session.read_buffer.advance_mut(len);
                            }

                            if <Buffer as Borrow<[u8]>>::borrow(&session.read_buffer) == b"PING\r\n" {
                                session.read_buffer.advance(6);
                                session.write_buffer.put_slice(b"PONG\r\n");

                                session.state = State::Write ;

                                let entry = opcode::Send::new(
                                    types::Fd(session.fd),
                                    session.write_buffer.read_ptr(),
                                    session.write_buffer.remaining() as _,
                                )


                                .build()
                                .user_data(token as _);

                                unsafe {
                                    if sq.push(&entry).is_err() {
                                        self.backlog.push_back(entry);
                                    }
                                }
                            } else {
                                let session = self.sessions.remove(token);
                                let fd = session.fd;
                                if self.session_tx.send(session).is_err() {
                                    unsafe {
                                        libc::close(fd);
                                    }
                                }
                            }
                        }
                    }
                    State::Write => {
                        let write_len = ret as usize;
                        // let buf = &mut self.buf_alloc[buf_index];

                        session.write_buffer.advance(write_len);

                        let entry = if session.write_buffer.remaining() == 0 {
                            // self.bufpool.push(buf_index);

                            session.state = State::Read;

                            opcode::Read::new(types::Fd(session.fd),
                                session.read_buffer.write_ptr(),
                                session.read_buffer.remaining_mut() as _)
                                .build()
                                .user_data(token as _)
                        } else {

                            session.state = State::Write;

                            opcode::Write::new(
                                types::Fd(session.fd),
                                session.read_buffer.write_ptr(),
                                session.read_buffer.remaining_mut() as _
                            )
                                .build()
                                .user_data(token as _)
                        };

                        unsafe {
                            if sq.push(&entry).is_err() {
                                self.backlog.push_back(entry);
                            }
                        }
                    }
                    _ => {
                        // this shouldn't happen here
                        panic!("unexpected session state");
                    }
                }
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

    pub fn run(mut self) {
        let (submitter, mut sq, mut cq) = self.ring.split();

        for _ in 0..1024 {
            let entry = opcode::Accept::new(types::Fd(self.listener.as_raw_fd()), ptr::null_mut(), ptr::null_mut())
                .build()
                .user_data(LISTENER_TOKEN);

            unsafe {
                match sq.push(&entry) {
                    Ok(_) => self.accept_backlog -= 1,
                    Err(_) => break,
                }
            }
        }

        sq.sync();

        loop {
            

            match submitter.submit_and_wait(1) {
                Ok(_) => (),
                Err(ref err) if err.raw_os_error() == Some(libc::EBUSY) => (),
                Err(err) => panic!("{}", err),
            }

            cq.sync();

            // handle backlog
            loop {
                if sq.is_full() {
                    match submitter.submit() {
                        Ok(_) => (),
                        Err(ref e) if e.raw_os_error() == Some(libc::EBUSY) => break,
                        Err(e) => panic!("{}", e),
                    }
                }

                sq.sync();

                match self.backlog.pop_front() {
                    Some(sqe) => unsafe {
                        let _ = sq.push(&sqe);
                    },
                    None => break,
                }
            }

            if let Ok(mut session) = self.session_rx.try_recv() {
                session.state = State::Shutdown;

                let fd = session.fd;
                let poll_token = self.sessions.insert(session);

                let entry = opcode::Shutdown::new(
                    types::Fd(fd),
                    libc::SHUT_RDWR,
                )
                .build()
                .user_data(poll_token as _);

                unsafe {
                    if sq.push(&entry).is_err() {
                        self.backlog.push_back(entry);
                    }
                }
            }

            for cqe in &mut cq {
                let ret = cqe.result();
                let token = cqe.user_data();

                if ret < 0 {
                    eprintln!(
                        "error: {:?}",
                        io::Error::from_raw_os_error(-ret)
                    );
                    continue;
                }

                // let token = &mut self.token_alloc[token_index];

                match token {
                    LISTENER_TOKEN => {
                        // println!("accept");

                        let entry = opcode::Accept::new(types::Fd(self.listener.as_raw_fd()), ptr::null_mut(), ptr::null_mut())
                            .build()
                            .user_data(LISTENER_TOKEN);

                        unsafe {
                            match sq.push(&entry) {
                                Ok(_) => self.accept_backlog -= 1,
                                Err(_) => break,
                            }
                        }

                        let fd = ret;

                        let session = Session {
                            read_buffer: Buffer::new(4096),
                            write_buffer: Buffer::new(4096),
                            state: State::Poll,
                            fd,
                        };

                        let poll_token = self.sessions.insert(session);

                        let event = opcode::PollAdd::new(types::Fd(fd), libc::POLLIN as _)
                            .build()
                            .user_data(poll_token as _);

                        unsafe {
                            if sq.push(&event).is_err() {
                                self.backlog.push_back(event);
                            }
                        }
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
                                let session = self.sessions.remove(token);
                                unsafe {
                                    libc::close(session.fd);
                                }
                            }
                            _ => {
                                panic!("unexpected session state");
                            }
                        }
                    }
                }
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
