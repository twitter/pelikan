// Copyright 2020 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use std::convert::TryInto;
use std::io::{Error, ErrorKind, Write};

use mio::net::TcpStream;
use rustcommon_buffer::*;
use rustls::ServerSession;
use rustls::Session as TlsSession;

use crate::*;

#[allow(dead_code)]
/// A `Session` is the complete state of a TCP stream
pub struct Session {
    token: Token,
    addr: SocketAddr,
    stream: TcpStream,
    state: State,
    buffer: Buffer,
    tls: Option<ServerSession>,
    metrics: Arc<Metrics<AtomicU64, AtomicU64>>,
}

impl Session {
    /// Create a new `Session` from an address, stream, and state
    pub fn new(
        addr: SocketAddr,
        stream: TcpStream,
        state: State,
        tls: Option<ServerSession>,
        metrics: Arc<Metrics<AtomicU64, AtomicU64>>,
    ) -> Self {
        let _ = metrics.increment_counter(&Stat::TcpAccept, 1);
        Self {
            token: Token(0),
            addr: addr,
            stream: stream,
            state,
            buffer: Buffer::with_capacity(1024, 1024),
            tls,
            metrics,
        }
    }

    pub fn buffer(&mut self) -> &mut Buffer {
        &mut self.buffer
    }

    /// Register the `Session` with the event loop
    pub fn register(&mut self, poll: &Poll) -> Result<(), std::io::Error> {
        let interest = self.readiness();
        poll.registry()
            .register(&mut self.stream, self.token, interest)
    }

    /// Deregister the `Session` from the event loop
    pub fn deregister(&mut self, poll: &Poll) -> Result<(), std::io::Error> {
        poll.registry().deregister(&mut self.stream)
    }

    /// Reregister the `Session` with the event loop
    pub fn reregister(&mut self, poll: &Poll) -> Result<(), std::io::Error> {
        let interest = self.readiness();
        poll.registry()
            .reregister(&mut self.stream, self.token, interest)
    }

    /// Reads from the stream into the session buffer
    pub fn read(&mut self) -> Result<Option<usize>, std::io::Error> {
        let _ = self.metrics.increment_counter(&Stat::TcpRecv, 1);
        if let Some(ref mut tls) = self.tls {
            match tls.read_tls(&mut self.stream) {
                Ok(0) => {
                    trace!("tls session read zero bytes bytes from stream");
                    Err(Error::new(ErrorKind::Other, "disconnected"))
                }
                Err(e) => {
                    if e.kind() == ErrorKind::WouldBlock {
                        trace!("would block, but might have data anyway...");
                        let _ = self.metrics.increment_counter(&Stat::SessionRecv, 1);
                        if tls.process_new_packets().is_ok() {
                            trace!("tls packet processing successful");
                            // now we read from the session
                            match self.buffer.read_from(tls) {
                                Ok(Some(0)) => {
                                    trace!("read 0 bytes from tls session, map to spurious wakeup");
                                    Ok(None)
                                }
                                Ok(Some(bytes)) => {
                                    trace!("session had: {} bytes to read", bytes);
                                    let _ = self.metrics.increment_counter(
                                        &Stat::SessionRecvByte,
                                        bytes.try_into().unwrap(),
                                    );
                                    Ok(Some(bytes))
                                }
                                Ok(None) => {
                                    trace!("got none back from tls session, spurious wakeup?");
                                    Ok(None)
                                }
                                Err(e) => {
                                    trace!("error reading from session");
                                    let _ = self.metrics.increment_counter(&Stat::SessionRecvEx, 1);
                                    Err(e)
                                }
                            }
                        } else {
                            trace!("tls error processing packets");
                            let _ = self.metrics.increment_counter(&Stat::SessionRecvEx, 1);
                            // try to write an error back to the client
                            let _ = tls.write_tls(&mut self.stream);
                            Err(Error::new(ErrorKind::Other, "tls error"))
                        }
                    } else {
                        trace!("tcp read error: {}", e);
                        let _ = self.metrics.increment_counter(&Stat::TcpRecvEx, 1);
                        // try to write an error back to the client
                        let _ = tls.write_tls(&mut self.stream);
                        Err(Error::new(ErrorKind::Other, "tls read error"))
                    }
                }
                Ok(bytes) => {
                    trace!("got {} bytes from tcpstream", bytes);
                    let _ = self
                        .metrics
                        .increment_counter(&Stat::TcpRecvByte, bytes.try_into().unwrap());
                    let _ = self.metrics.increment_counter(&Stat::SessionRecv, 1);
                    if tls.process_new_packets().is_ok() {
                        trace!("tls packet processing successful");
                        // now we read from the session
                        match self.buffer.read_from(tls) {
                            Ok(Some(0)) => {
                                trace!("read 0 bytes from tls session, map to spurious wakeup");
                                Ok(None)
                            }
                            Ok(Some(bytes)) => {
                                trace!("session had: {} bytes to read", bytes);
                                let _ = self.metrics.increment_counter(
                                    &Stat::SessionRecvByte,
                                    bytes.try_into().unwrap(),
                                );
                                Ok(Some(bytes))
                            }
                            Ok(None) => {
                                trace!("got none back from tls session, spurious wakeup?");
                                Ok(None)
                            }
                            Err(e) => {
                                trace!("error reading from session");
                                let _ = self.metrics.increment_counter(&Stat::SessionRecvEx, 1);
                                Err(e)
                            }
                        }
                    } else {
                        trace!("tls error processing packets");
                        let _ = self.metrics.increment_counter(&Stat::SessionRecvEx, 1);
                        // try to write an error back to the client
                        let _ = tls.write_tls(&mut self.stream);
                        Err(Error::new(ErrorKind::Other, "tls error"))
                    }
                }
            }
        } else {
            match self.buffer.read_from(&mut self.stream) {
                Ok(Some(0)) => Ok(Some(0)),
                Ok(Some(bytes)) => {
                    let _ = self
                        .metrics
                        .increment_counter(&Stat::TcpRecvByte, bytes.try_into().unwrap());
                    Ok(Some(bytes))
                }
                Ok(None) => Ok(None),
                Err(e) => {
                    let _ = self.metrics.increment_counter(&Stat::TcpRecvEx, 1);
                    Err(e)
                }
            }
        }
    }

    /// Write to the session buffer
    pub fn write(&mut self, buf: &[u8]) -> Result<usize, std::io::Error> {
        self.buffer.write(buf)
    }

    /// Flush the session buffer to the stream
    pub fn flush(&mut self) -> Result<Option<usize>, std::io::Error> {
        if let Some(ref mut tls) = self.tls {
            let _ = self.metrics.increment_counter(&Stat::SessionSend, 1);
            match self.buffer.write_to(tls) {
                Ok(Some(bytes)) => {
                    let _ = self
                        .metrics
                        .increment_counter(&Stat::SessionSendByte, bytes.try_into().unwrap());
                    Ok(Some(bytes))
                }
                Ok(None) => Ok(None),
                Err(e) => {
                    let _ = self.metrics.increment_counter(&Stat::SessionSendEx, 1);
                    Err(e)
                }
            }?;
            if tls.wants_write() {
                let _ = self.metrics.increment_counter(&Stat::TcpSend, 1);
                match tls.write_tls(&mut self.stream) {
                    Ok(bytes) => {
                        let _ = self
                            .metrics
                            .increment_counter(&Stat::TcpSendByte, bytes.try_into().unwrap());
                        Ok(Some(bytes))
                    }
                    Err(e) => {
                        let _ = self.metrics.increment_counter(&Stat::TcpSendEx, 1);
                        Err(e)
                    }
                }
            } else {
                Ok(None)
            }
        } else {
            let _ = self.metrics.increment_counter(&Stat::TcpSend, 1);
            match self.buffer.write_to(&mut self.stream) {
                Ok(Some(bytes)) => {
                    let _ = self
                        .metrics
                        .increment_counter(&Stat::TcpSendByte, bytes.try_into().unwrap());
                    Ok(Some(bytes))
                }
                Ok(None) => Ok(None),
                Err(e) => {
                    let _ = self.metrics.increment_counter(&Stat::TcpSendEx, 1);
                    Err(e)
                }
            }
        }
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
        if let Some(ref tls) = self.tls {
            if tls.wants_write() || self.buffer.write_pending() != 0 {
                Interest::READABLE | Interest::WRITABLE
            } else {
                Interest::READABLE
            }
        } else {
            if self.buffer.write_pending() != 0 {
                Interest::READABLE | Interest::WRITABLE
            } else {
                Interest::READABLE
            }
        }
    }

    pub fn is_handshaking(&self) -> bool {
        if let Some(ref tls) = self.tls {
            tls.is_handshaking()
        } else {
            false
        }
    }

    pub fn close(&mut self) {
        trace!("closing session");
        let _ = self.metrics.increment_counter(&Stat::TcpClose, 1);
        let _ = self.stream.shutdown(std::net::Shutdown::Both);
        self.tls = None;
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum State {
    Handshaking,
    Established,
}
