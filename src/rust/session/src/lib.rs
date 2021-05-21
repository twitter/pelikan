// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! This crate provides buffered TCP sessions with or without TLS which can be
//! used with [`::mio`]. TLS/SSL is provided by BoringSSL with the [`::boring`]
//! crate.

#[macro_use]
extern crate rustcommon_fastmetrics;

mod buffer;
mod stream;
mod tcp_stream;

use std::borrow::Borrow;
use std::io::{ErrorKind, Read, Write};
use std::net::SocketAddr;

use boring::ssl::{MidHandshakeSslStream, SslStream};
use bytes::Buf;
use common::traits::ExtendFromSlice;
use metrics::Stat;
use mio::event::Source;
use mio::{Interest, Poll, Token};

use buffer::Buffer;
use stream::Stream;

pub use tcp_stream::TcpStream;

pub const MIN_BUFFER_SIZE: usize = 1024; // 1 KiB

// TODO(bmartin): implement connect/reconnect so we can use this in clients too.
/// The core `Session` type which represents a TCP stream (with or without TLS),
/// the session buffer, the mio [`::mio::Token`],
pub struct Session {
    token: Token,
    // TODO(bmartin): remove this lint exception after implementing connect
    #[allow(dead_code)]
    addr: SocketAddr,
    stream: Stream,
    read_buffer: Buffer,
    write_buffer: Buffer,
    tmp_buffer: [u8; MIN_BUFFER_SIZE],
}

impl Session {
    /// Create a new `Session` representing a plain `TcpStream`
    pub fn plain(addr: SocketAddr, stream: TcpStream) -> Self {
        Self::new(addr, Stream::plain(stream))
    }

    /// Create a new `Session` representing a negotiated `SslStream`
    pub fn tls(addr: SocketAddr, stream: SslStream<TcpStream>) -> Self {
        Self::new(addr, Stream::tls(stream))
    }

    /// Create a new `Session` representing a `MidHandshakeSslStream`
    pub fn handshaking(addr: SocketAddr, stream: MidHandshakeSslStream<TcpStream>) -> Self {
        Self::new(addr, Stream::handshaking(stream))
    }

    /// Create a new `Session`
    fn new(addr: SocketAddr, stream: Stream) -> Self {
        increment_counter!(&Stat::TcpAccept);
        Self {
            token: Token(0),
            addr,
            stream,
            read_buffer: Buffer::with_capacity(MIN_BUFFER_SIZE),
            write_buffer: Buffer::with_capacity(MIN_BUFFER_SIZE),
            tmp_buffer: [0; MIN_BUFFER_SIZE],
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

    /// Reads from the stream into the session buffer
    pub fn read(&mut self) -> Result<Option<usize>, std::io::Error> {
        increment_counter!(&Stat::SessionRecv);
        let mut total_bytes = 0;
        loop {
            match self.stream.read(&mut self.tmp_buffer) {
                Ok(0) => {
                    // Stream is disconnected, stop reading
                    break;
                }
                Ok(bytes) => {
                    self.read_buffer.extend(&self.tmp_buffer[0..bytes]);
                    total_bytes += bytes;
                    if bytes < self.tmp_buffer.len() {
                        // we read less than the temp buffer size, next read
                        // is likely to block so we can stop reading.
                        break;
                    }
                }
                Err(e) => {
                    if e.kind() == ErrorKind::WouldBlock {
                        // check if we blocked on first read or subsequent read.
                        // if blocked on a subsequent read, we stop reading and
                        // allow the function to return the number of bytes read
                        // until now.
                        if total_bytes == 0 {
                            return Ok(None);
                        } else {
                            break;
                        }
                    } else {
                        increment_counter!(&Stat::SessionRecvEx);
                        return Err(e);
                    }
                }
            }
        }
        increment_counter_by!(&Stat::SessionRecvByte, total_bytes as u64);
        Ok(Some(total_bytes))
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
    pub fn read_pending(&mut self) -> usize {
        self.read_buffer.len()
    }

    /// Returns the number of bytes in the write buffer
    pub fn write_pending(&mut self) -> usize {
        self.write_buffer.len()
    }

    /// Borrow the contents of the read buffer without consuming or copying the
    /// buffer contents.
    pub fn peek(&self) -> &[u8] {
        self.read_buffer.borrow()
    }

    /// Consume the specified number of bytes from the read buffer so that they
    /// will not be returned by the next call to `peek()`.
    pub fn consume(&mut self, bytes: usize) {
        self.read_buffer.inner.advance(bytes);
    }
}

impl Write for Session {
    fn write(&mut self, src: &[u8]) -> Result<usize, std::io::Error> {
        self.write_buffer.extend(src);
        Ok(src.len())
    }

    fn flush(&mut self) -> Result<(), std::io::Error> {
        increment_counter!(&Stat::SessionSend);
        match self.stream.write((self.write_buffer).borrow()) {
            Ok(0) => Ok(()),
            Ok(bytes) => {
                increment_counter_by!(&Stat::SessionSendByte, bytes as u64);
                self.write_buffer.advance(bytes);
                Ok(())
            }
            Err(e) => {
                increment_counter!(&Stat::SessionSendEx);
                Err(e)
            }
        }
    }
}
