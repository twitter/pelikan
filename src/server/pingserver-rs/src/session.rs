// Copyright 2020 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use mio::net::TcpStream;
use crate::*;

use crate::buffer::Buffer;

use std::io::Write;

#[allow(dead_code)]
/// A `Session` is the complete state of a TCP stream
pub struct Session {
    token: Token,
    addr: SocketAddr,
    stream: TcpStream,
    state: State,
    buffer: Buffer,
}

impl Session {
    /// Create a new `Session` from an address, stream, and state
    pub fn new(addr: SocketAddr, stream: TcpStream, state: State) -> Self {
        Self {
            token: Token(0),
            addr,
            stream,
            state,
            buffer: Buffer::new(1024, 1024),
        }
    }

    /// Register the `Session` with the event loop
    pub fn register(&mut self, poll: &Poll) -> Result<(), std::io::Error> {
        let interest = self.readiness();
        poll.registry().register(&mut self.stream, self.token, interest)
    }

    /// Deregister the `Session` from the event loop
    pub fn deregister(&mut self, poll: &Poll) -> Result<(), std::io::Error> {
        poll.registry().deregister(&mut self.stream)
    }

    /// Reregister the `Session` with the event loop
    pub fn reregister(&mut self, poll: &Poll) -> Result<(), std::io::Error> {
        let interest = self.readiness();
        poll.registry().reregister(&mut self.stream, self.token, interest)
    }

    /// Reads from the stream into the session buffer
    pub fn read(&mut self) -> Result<Option<usize>, std::io::Error> {
        self.buffer.read_from(&mut self.stream)
    }

    /// Get a reference to the contents of the receive buffer
    pub fn rx_buffer(&self) -> &[u8] {
        self.buffer.rx_buffer()
    }

    /// Return true if there are still bytes in the tx buffer
    pub fn tx_pending(&self) -> bool {
        self.buffer.tx_pending()
    }

    /// Clear the buffer
    pub fn clear_buffer(&mut self) {
        self.buffer.clear()
    }

    /// Write to the session buffer
    pub fn write(&mut self, buf: &[u8]) -> Result<usize, std::io::Error> {
        self.buffer.write(buf)
    }

    /// Flush the session buffer to the stream
    pub fn flush(&mut self) -> Result<Option<usize>, std::io::Error> {
        self.buffer.write_to(&mut self.stream)
    }

    /// Set the state of the session
    pub fn set_state(&mut self, state: State) {
        // TODO(bmartin): validate state transitions
        self.state = state;
    }

    /// Set the token which is used with the event loop
    pub fn set_token(&mut self, token: Token) {
        self.token = token;
    }

    /// Get the set of readiness events the session is waiting for
    fn readiness(&self) -> Interest {
        match self.state {
            State::Reading => Interest::READABLE,
            State::Writing => Interest::WRITABLE,
        }
    }
}

pub enum State {
    Reading,
    Writing,
}
