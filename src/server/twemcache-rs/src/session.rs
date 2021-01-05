// Copyright 2020 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::*;

use boring::ssl::{HandshakeError, MidHandshakeSslStream, SslStream};
use bytes::{Buf, BytesMut};
use metrics::Stat;
// use mio::net::TcpStream;

use std::borrow::Borrow;

use std::io::{Error, ErrorKind, Read, Write};

pub struct TcpStream {
    inner: mio::net::TcpStream,
}

impl From<mio::net::TcpStream> for TcpStream {
    fn from(other: mio::net::TcpStream) -> Self {
        Self { inner: other }
    }
}

impl TcpStream {
    fn shutdown(&self, how: std::net::Shutdown) -> Result<(), std::io::Error> {
        self.inner.shutdown(how)
    }
}

impl Read for TcpStream {
    fn read(&mut self, buf: &mut [u8]) -> std::result::Result<usize, std::io::Error> {
        let result = self.inner.read(buf);
        if let Ok(bytes) = result {
            increment_counter_by!(&Stat::TcpRecvByte, bytes as u64);
        }
        result
    }
}

impl Write for TcpStream {
    fn write(&mut self, buf: &[u8]) -> std::result::Result<usize, std::io::Error> {
        let result = self.inner.write(buf);
        if let Ok(bytes) = result {
            increment_counter_by!(&Stat::TcpSendByte, bytes as u64);
        }
        result
    }
    fn flush(&mut self) -> std::result::Result<(), std::io::Error> {
        self.inner.flush()
    }
}

impl mio::event::Source for TcpStream {
    fn register(
        &mut self,
        registry: &mio::Registry,
        token: mio::Token,
        interest: mio::Interest,
    ) -> std::result::Result<(), std::io::Error> {
        self.inner.register(registry, token, interest)
    }

    fn reregister(
        &mut self,
        registry: &mio::Registry,
        token: mio::Token,
        interest: mio::Interest,
    ) -> std::result::Result<(), std::io::Error> {
        self.inner.reregister(registry, token, interest)
    }

    fn deregister(&mut self, registry: &mio::Registry) -> std::result::Result<(), std::io::Error> {
        self.inner.deregister(registry)
    }
}

pub enum Stream {
    Plain(TcpStream),
    Tls(SslStream<TcpStream>),
    Handshaking(MidHandshakeSslStream<TcpStream>),
}

#[allow(dead_code)]
/// A `Session` is the complete state of a TCP stream
pub struct Session {
    token: Token,
    addr: SocketAddr,
    stream: Option<Stream>,
    pub read_buffer: BytesMut,
    pub write_buffer: Option<BytesMut>,
    tmp_buffer: [u8; 1024],
}

impl Session {
    /// Create a new `Session` representing a plain `TcpStream`
    pub fn plain(addr: SocketAddr, stream: TcpStream) -> Self {
        Self::new(addr, Stream::Plain(stream))
    }

    /// Create a new `Session` representing a negotiated `SslStream`
    pub fn tls(addr: SocketAddr, stream: SslStream<TcpStream>) -> Self {
        Self::new(addr, Stream::Tls(stream))
    }

    /// Create a new `Session` representing a `MidHandshakeSslStream`
    pub fn handshaking(addr: SocketAddr, stream: MidHandshakeSslStream<TcpStream>) -> Self {
        Self::new(addr, Stream::Handshaking(stream))
    }

    // Create a new `Session`
    fn new(addr: SocketAddr, stream: Stream) -> Self {
        increment_counter!(&Stat::TcpAccept);
        Self {
            token: Token(0),
            addr,
            stream: Some(stream),
            read_buffer: BytesMut::with_capacity(1024),
            write_buffer: Some(BytesMut::with_capacity(1024)),
            tmp_buffer: [0; 1024],
        }
    }

    /// Register the `Session` with the event loop
    pub fn register(&mut self, poll: &Poll) -> Result<(), std::io::Error> {
        let interest = self.readiness();
        match &mut self.stream {
            Some(Stream::Plain(s)) => poll.registry().register(s, self.token, interest),
            Some(Stream::Tls(s)) => poll.registry().register(s.get_mut(), self.token, interest),
            Some(Stream::Handshaking(s)) => {
                poll.registry().register(s.get_mut(), self.token, interest)
            }
            None => Err(Error::new(ErrorKind::Other, "session has no stream")),
        }
    }

    /// Deregister the `Session` from the event loop
    pub fn deregister(&mut self, poll: &Poll) -> Result<(), std::io::Error> {
        match &mut self.stream {
            Some(Stream::Plain(s)) => poll.registry().deregister(s),
            Some(Stream::Tls(s)) => poll.registry().deregister(s.get_mut()),
            Some(Stream::Handshaking(s)) => poll.registry().deregister(s.get_mut()),
            None => Err(Error::new(ErrorKind::Other, "session has no stream")),
        }
    }

    /// Reregister the `Session` with the event loop
    pub fn reregister(&mut self, poll: &Poll) -> Result<(), std::io::Error> {
        let interest = self.readiness();
        match &mut self.stream {
            Some(Stream::Plain(s)) => poll.registry().reregister(s, self.token, interest),
            Some(Stream::Tls(s)) => poll
                .registry()
                .reregister(s.get_mut(), self.token, interest),
            Some(Stream::Handshaking(s)) => {
                poll.registry()
                    .reregister(s.get_mut(), self.token, interest)
            }
            None => Err(Error::new(ErrorKind::Other, "session has no stream")),
        }
    }

    /// Reads from the stream into the session buffer
    pub fn read(&mut self) -> Result<Option<usize>, std::io::Error> {
        increment_counter!(&Stat::SessionRecv);
        let mut total_bytes = 0;
        loop {
            let read_result = match &mut self.stream {
                Some(Stream::Plain(s)) => s.read(&mut self.tmp_buffer),
                Some(Stream::Tls(s)) => s.read(&mut self.tmp_buffer),
                Some(Stream::Handshaking(_)) => {
                    return Ok(None);
                }
                _ => {
                    return Err(Error::new(ErrorKind::Other, "session has no stream"));
                }
            };
            match read_result {
                Ok(0) => {
                    // Stream is disconnected, stop reading
                    break;
                }
                Ok(bytes) => {
                    self.read_buffer
                        .extend_from_slice(&self.tmp_buffer[0..bytes]);
                    total_bytes += bytes;
                    if bytes < self.tmp_buffer.len() {
                        // we read less than the temp buffer size, next read
                        // is likely to block so we can stop reading.
                        break;
                    }
                }
                Err(e) => {
                    if e.kind() == ErrorKind::WouldBlock {
                        // check if we blocked on the first read or on a
                        // subsequent read. This is just an easy way to
                        // differentiate between HUP on first read and block on
                        // first read.
                        if total_bytes == 0 {
                            return Ok(None);
                        } else {
                            break;
                        }
                    } else {
                        trace!("error reading from session");
                        increment_counter!(&Stat::SessionRecvEx);
                        return Err(e);
                    }
                }
            }
        }
        increment_counter_by!(&Stat::SessionRecvByte, total_bytes as u64);
        Ok(Some(total_bytes))
    }

    /// Flush the session buffer to the stream
    pub fn flush(&mut self) -> Result<Option<usize>, std::io::Error> {
        if let Some(ref mut write_buffer) = self.write_buffer {
            increment_counter!(&Stat::SessionSend);
            let write_result = match &mut self.stream {
                Some(Stream::Plain(s)) => s.write((*write_buffer).borrow()),
                Some(Stream::Tls(s)) => s.write((*write_buffer).borrow()),
                Some(Stream::Handshaking(_)) => {
                    return Ok(None);
                }
                None => {
                    return Err(Error::new(ErrorKind::Other, "session has no stream"));
                }
            };
            match write_result {
                Ok(0) => Ok(Some(0)),
                Ok(bytes) => {
                    increment_counter_by!(&Stat::SessionSendByte, bytes as u64);
                    write_buffer.advance(bytes);
                    Ok(Some(bytes))
                }
                Err(e) => {
                    increment_counter!(&Stat::SessionSendEx);
                    Err(e)
                }
            }
        } else {
            Err(Error::new(ErrorKind::Other, "session has no write buffer"))
        }
    }

    /// Set the token which is used with the event loop
    pub fn set_token(&mut self, token: Token) {
        self.token = token;
    }

    /// Get the set of readiness events the session is waiting for
    fn readiness(&self) -> Interest {
        if self
            .write_buffer
            .as_ref()
            .map(|buf| buf.is_empty())
            .unwrap_or(true)
        {
            Interest::READABLE
        } else {
            Interest::READABLE | Interest::WRITABLE
        }
    }

    pub fn is_handshaking(&self) -> bool {
        matches!(self.stream, Some(Stream::Handshaking(_)))
    }

    pub fn do_handshake(&mut self) -> Result<(), std::io::Error> {
        if let Some(Stream::Handshaking(stream)) = self.stream.take() {
            let ret;
            let result = stream.handshake();
            self.stream = match result {
                Ok(established) => {
                    ret = Ok(());
                    Some(Stream::Tls(established))
                }
                Err(HandshakeError::WouldBlock(handshaking)) => {
                    ret = Err(Error::new(ErrorKind::WouldBlock, "handshake would block"));
                    Some(Stream::Handshaking(handshaking))
                }
                _ => {
                    ret = Err(Error::new(ErrorKind::Other, "handshaking error"));
                    None
                }
            };
            ret
        } else {
            Err(Error::new(ErrorKind::Other, "session is not in handshaking state"))
        }
    }

    pub fn close(&mut self) {
        trace!("closing session");
        increment_counter!(&Stat::TcpClose);
        if let Some(stream) = self.stream.take() {
            self.stream = match stream {
                Stream::Plain(s) => {
                    let _ = s.shutdown(std::net::Shutdown::Both);
                    Some(Stream::Plain(s))
                }
                Stream::Tls(mut s) => {
                    // TODO(bmartin): session resume requires that a full graceful
                    // shutdown occurs
                    let _ = s.shutdown();
                    Some(Stream::Tls(s))
                }
                Stream::Handshaking(mut s) => {
                    // since we don't have a fully established session, just
                    // shutdown the underlying tcp stream
                    let _ = s.get_mut().shutdown(std::net::Shutdown::Both);
                    Some(Stream::Handshaking(s))
                }
            }
        }
    }

    pub fn read_pending(&self) -> usize {
        self.read_buffer.len()
    }

    pub fn write_pending(&self) -> usize {
        self.write_buffer.as_ref().map(|buf| buf.len()).unwrap_or(0)
    }
}
