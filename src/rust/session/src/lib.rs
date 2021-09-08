// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! This crate provides buffered TCP sessions with or without TLS which can be
//! used with [`::mio`]. TLS/SSL is provided by BoringSSL with the [`::boring`]
//! crate.

mod buffer;
mod stream;
mod tcp_stream;

use std::borrow::Borrow;
use std::borrow::BorrowMut;
use std::io::BufRead;
use std::io::{ErrorKind, Read, Write};
use std::net::SocketAddr;

use boring::ssl::{MidHandshakeSslStream, SslStream};
use metrics::{static_metrics, Counter};
use mio::event::Source;
use mio::{Interest, Poll, Token};

use buffer::Buffer;
use stream::Stream;

pub use tcp_stream::TcpStream;

static_metrics! {
    static TCP_ACCEPT: Counter;
    static TCP_CLOSE: Counter;
    static TCP_RECV_BYTE: Counter;
    static TCP_SEND_BYTE: Counter;
    static TCP_SEND_PARTIAL: Counter;

    static SESSION_RECV: Counter;
    static SESSION_RECV_EX: Counter;
    static SESSION_RECV_BYTE: Counter;
    static SESSION_SEND: Counter;
    static SESSION_SEND_EX: Counter;
    static SESSION_SEND_BYTE: Counter;
}

// TODO(bmartin): implement connect/reconnect so we can use this in clients too.
/// The core `Session` type which represents a TCP stream (with or without TLS),
/// the session buffer, the mio [`::mio::Token`],
pub struct Session {
    token: Token,
    stream: Stream,
    read_buffer: Buffer,
    write_buffer: Buffer,
    min_capacity: usize,
    max_capacity: usize,
}

impl Session {
    /// Create a new `Session` with  representing a plain `TcpStream` with
    /// internal buffers which can hold up to capacity bytes without
    /// reallocating.
    pub fn plain_with_capacity(
        stream: TcpStream,
        min_capacity: usize,
        max_capacity: usize,
    ) -> Self {
        Self::new(Stream::plain(stream), min_capacity, max_capacity)
    }

    /// Create a new `Session` representing a negotiated `SslStream`
    pub fn tls_with_capacity(
        stream: SslStream<TcpStream>,
        min_capacity: usize,
        max_capacity: usize,
    ) -> Self {
        Self::new(Stream::tls(stream), min_capacity, max_capacity)
    }

    /// Create a new `Session` representing a `MidHandshakeSslStream`
    pub fn handshaking_with_capacity(
        stream: MidHandshakeSslStream<TcpStream>,
        min_capacity: usize,
        max_capacity: usize,
    ) -> Self {
        Self::new(Stream::handshaking(stream), min_capacity, max_capacity)
    }

    /// Create a new `Session`
    fn new(stream: Stream, min_capacity: usize, max_capacity: usize) -> Self {
        TCP_ACCEPT.increment();
        Self {
            token: Token(0),
            stream,
            read_buffer: Buffer::with_capacity(min_capacity),
            write_buffer: Buffer::with_capacity(min_capacity),
            min_capacity,
            max_capacity,
        }
    }

    /// Register the `Session` with the event loop
    pub fn register(&mut self, poll: &Poll) -> Result<(), std::io::Error> {
        let interest = self.readiness();
        self.stream.register(poll.registry(), self.token, interest)
    }

    /// Deregister the `Session` from the event loop
    pub fn deregister(&mut self, poll: &Poll) -> Result<(), std::io::Error> {
        self.stream.deregister(poll.registry())
    }

    /// Reregister the `Session` with the event loop
    pub fn reregister(&mut self, poll: &Poll) -> Result<(), std::io::Error> {
        let interest = self.readiness();
        self.stream
            .reregister(poll.registry(), self.token, interest)
    }

    /// Get the token which is used with the event loop
    pub fn token(&self) -> Token {
        self.token
    }

    /// Set the token which is used with the event loop
    pub fn set_token(&mut self, token: Token) {
        self.token = token;
    }

    /// Get the set of readiness events the session is waiting for
    fn readiness(&self) -> Interest {
        if self.write_buffer.is_empty() {
            Interest::READABLE
        } else {
            Interest::READABLE | Interest::WRITABLE
        }
    }

    /// Returns a boolean which indicates if the session is handshaking
    pub fn is_handshaking(&self) -> bool {
        self.stream.is_handshaking()
    }

    /// Drives the handshake for the session. A successful result indicates that
    /// the session hadshake is completed successfully. The error result should
    /// be checked to determine if the operation would block, resulted in some
    /// unrecoverable error, or if the session was not in a handshaking state
    /// when this was called.
    pub fn do_handshake(&mut self) -> Result<(), std::io::Error> {
        self.stream.do_handshake()
    }

    /// Closes the session and the underlying stream.
    pub fn close(&mut self) {
        self.stream.close();
    }

    /// Returns the number of bytes in the read buffer
    pub fn read_pending(&self) -> usize {
        self.read_buffer.len()
    }

    /// Returns the number of bytes in the write buffer
    pub fn write_pending(&self) -> usize {
        self.write_buffer.len()
    }

    /// Returns the number of bytes free in the write buffer relative to the
    /// minimum buffer size. This allows us to use it as a signal that we should
    /// apply some backpressure on handling requests for the session.
    pub fn write_capacity(&self) -> usize {
        self.min_capacity.saturating_sub(self.write_pending())
    }

    /// Returns a reference to the internally buffered data.
    ///
    /// Unlike [`fill_buf`], this will not attempt to fill the buffer if it is
    /// empty.
    ///
    /// [`fill_buf`]: BufRead::fill_buf
    pub fn buffer(&self) -> &[u8] {
        self.read_buffer.borrow()
    }

    pub fn peer_addr(&self) -> Result<SocketAddr, std::io::Error> {
        self.stream.peer_addr()
    }
}

impl Read for Session {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error> {
        if self.read_buffer.is_empty() {
            self.fill_buf()?;
        }
        let bytes = std::cmp::min(buf.len(), self.read_buffer.len());
        let buffer: &[u8] = self.read_buffer.borrow();
        buf[0..bytes].copy_from_slice(&buffer[0..bytes]);
        self.consume(bytes);
        Ok(bytes)
    }
}

impl BufRead for Session {
    fn fill_buf(&mut self) -> Result<&[u8], std::io::Error> {
        SESSION_RECV.increment();
        let mut total_bytes = 0;
        loop {
            if self.read_buffer.len() == self.max_capacity {
                return Err(std::io::Error::new(ErrorKind::Other, "buffer full"));
            }

            // reserve additional space in the buffer if needed
            if self.read_buffer.available_capacity() == 0 {
                self.read_buffer.reserve(self.min_capacity);
            }

            match self.stream.read(self.read_buffer.borrow_mut()) {
                Ok(0) => {
                    // Stream is disconnected, stop reading
                    break;
                }
                Ok(bytes) => {
                    self.read_buffer.increase_len(bytes);
                    total_bytes += bytes;
                }
                Err(e) => {
                    if e.kind() == ErrorKind::WouldBlock {
                        // check if we blocked on first read or subsequent read.
                        // if blocked on a subsequent read, we stop reading and
                        // allow the function to return the number of bytes read
                        // until now.
                        if total_bytes == 0 {
                            return Err(e);
                        } else {
                            break;
                        }
                    } else {
                        SESSION_RECV_EX.increment();
                        return Err(e);
                    }
                }
            }
        }
        SESSION_RECV_BYTE.add(total_bytes as _);
        Ok(self.read_buffer.borrow())
    }

    fn consume(&mut self, amt: usize) {
        self.read_buffer.consume(amt);
    }
}

impl Write for Session {
    fn write(&mut self, src: &[u8]) -> Result<usize, std::io::Error> {
        self.write_buffer.reserve(src.len());
        self.write_buffer.extend_from_slice(src);
        Ok(src.len())
    }

    fn flush(&mut self) -> Result<(), std::io::Error> {
        SESSION_SEND.increment();
        match self.stream.write((self.write_buffer).borrow()) {
            Ok(0) => Ok(()),
            Ok(bytes) => {
                SESSION_SEND_BYTE.add(bytes as _);
                self.write_buffer.consume(bytes);
                self.stream.flush()
            }
            Err(e) => {
                SESSION_SEND_EX.increment();
                Err(e)
            }
        }
    }
}

metrics::test_no_duplicates!();
