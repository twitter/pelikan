// Copyright 2020 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::*;
use boring::ssl::{HandshakeError, MidHandshakeSslStream, SslStream};
use mio::net::TcpStream;
use rustcommon_buffer::*;
use rustcommon_fastmetrics::metrics;
use std::io::{Error, ErrorKind, Write};

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
    buffer: Buffer,
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

    /// Create a new `Session` from an address, stream, and state
    fn new(addr: SocketAddr, stream: Stream) -> Self {
        increment_counter!(&Stat::TcpAccept);
        Self {
            token: Token(0),
            addr,
            stream: Some(stream),
            buffer: Buffer::with_capacity(1024, 1024),
        }
    }

    pub fn buffer(&mut self) -> &mut Buffer {
        &mut self.buffer
    }

    /// Register the `Session` with the event loop
    pub fn register(&mut self, poll: &Poll) -> Result<(), std::io::Error> {
        let interest = self.readiness();
        let tcp_stream = match &mut self.stream {
            Some(Stream::Plain(s)) => s,
            Some(Stream::Tls(s)) => s.get_mut(),
            Some(Stream::Handshaking(s)) => s.get_mut(),
            None => {
                return Err(Error::new(ErrorKind::Other, "session has no stream"));
            }
        };
        poll.registry().register(tcp_stream, self.token, interest)
    }

    /// Deregister the `Session` from the event loop
    pub fn deregister(&mut self, poll: &Poll) -> Result<(), std::io::Error> {
        let tcp_stream = match &mut self.stream {
            Some(Stream::Plain(s)) => s,
            Some(Stream::Tls(s)) => s.get_mut(),
            Some(Stream::Handshaking(s)) => s.get_mut(),
            None => {
                return Err(Error::new(ErrorKind::Other, "session has no stream"));
            }
        };
        poll.registry().deregister(tcp_stream)
    }

    /// Reregister the `Session` with the event loop
    pub fn reregister(&mut self, poll: &Poll) -> Result<(), std::io::Error> {
        let interest = self.readiness();
        let tcp_stream = match &mut self.stream {
            Some(Stream::Plain(s)) => s,
            Some(Stream::Tls(s)) => s.get_mut(),
            Some(Stream::Handshaking(s)) => s.get_mut(),
            None => {
                return Err(Error::new(ErrorKind::Other, "session has no stream"));
            }
        };
        poll.registry().reregister(tcp_stream, self.token, interest)
    }

    /// Reads from the stream into the session buffer
    pub fn read(&mut self) -> Result<Option<usize>, std::io::Error> {
        increment_counter!(&Stat::TcpRecv);

        let read_result = match &mut self.stream {
            Some(Stream::Plain(s)) => self.buffer.read_from(s),
            Some(Stream::Tls(s)) => self.buffer.read_from(s),
            Some(Stream::Handshaking(_)) => Ok(None),
            None => Err(Error::new(ErrorKind::Other, "session has no stream")),
        };

        match read_result {
            Ok(Some(0)) => Ok(Some(0)),
            Ok(Some(bytes)) => {
                increment_counter_by!(&Stat::TcpRecvByte, bytes as u64);
                Ok(Some(bytes))
            }
            Ok(None) => Ok(None),
            Err(e) => {
                increment_counter!(&Stat::TcpRecvEx);
                Err(e)
            }
        }
    }

    /// Write to the session buffer
    pub fn write(&mut self, buf: &[u8]) -> Result<usize, std::io::Error> {
        self.buffer.write(buf)
    }

    /// Flush the session buffer to the stream
    pub fn flush(&mut self) -> Result<Option<usize>, std::io::Error> {
        let write_result = match &mut self.stream {
            Some(Stream::Plain(s)) => self.buffer.write_to(s),
            Some(Stream::Tls(s)) => self.buffer.write_to(s),
            Some(Stream::Handshaking(_)) => Ok(None),
            None => Err(Error::new(ErrorKind::Other, "session has no stream")),
        };

        match write_result {
            Ok(Some(bytes)) => {
                increment_counter_by!(&Stat::TcpSendByte, bytes as u64);
                Ok(Some(bytes))
            }
            Ok(None) => Ok(None),
            Err(e) => {
                increment_counter!(&Stat::TcpSendEx);
                Err(e)
            }
        }
    }

    /// Set the token which is used with the event loop
    pub fn set_token(&mut self, token: Token) {
        self.token = token;
    }

    /// Get the set of readiness events the session is waiting for
    fn readiness(&self) -> Interest {
        if self.buffer.write_pending() != 0 {
            Interest::READABLE | Interest::WRITABLE
        } else {
            Interest::READABLE
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
            Err(Error::new(ErrorKind::Other, "session contains no stream"))
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
}
