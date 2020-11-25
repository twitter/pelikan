// Copyright 2020 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

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
}

impl Session {
    /// Create a new `Session` from an address, stream, and state
    pub fn new(
        addr: SocketAddr,
        stream: TcpStream,
        state: State,
        tls: Option<ServerSession>,
    ) -> Self {
        Self {
            token: Token(0),
            addr: addr,
            stream: stream,
            state,
            buffer: Buffer::with_capacity(1024, 1024),
            tls,
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
        if let Some(ref mut tls) = self.tls {
            match tls.read_tls(&mut self.stream) {
                Ok(0) => {
                    trace!("tls session read zero bytes bytes from stream");
                    Err(Error::new(ErrorKind::Other, "disconnected"))
                }
                Err(e) => {
                    if e.kind() == ErrorKind::WouldBlock {
                        trace!("would block, but might have data anyway...");
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
                                    Ok(Some(bytes))
                                }
                                Ok(None) => {
                                    trace!("got none back from tls session, spurious wakeup?");
                                    Ok(None)
                                }
                                Err(e) => {
                                    trace!("error reading from session");
                                    Err(e)
                                }
                            }
                        } else {
                            trace!("tls error processing packets");
                            // try to write an error back to the client
                            let _ = tls.write_tls(&mut self.stream);
                            Err(Error::new(ErrorKind::Other, "tls error"))
                        }
                    } else {
                        trace!("tls read error: {}", e);
                        // try to write an error back to the client
                        let _ = tls.write_tls(&mut self.stream);
                        Err(Error::new(ErrorKind::Other, "tls read error"))
                    }
                }
                Ok(bytes) => {
                    trace!("got {} bytes from tcpstream", bytes);
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
                                Ok(Some(bytes))
                            }
                            Ok(None) => {
                                trace!("got none back from tls session, spurious wakeup?");
                                Ok(None)
                            }
                            Err(e) => {
                                trace!("error reading from session");
                                Err(e)
                            }
                        }
                    } else {
                        trace!("tls error processing packets");
                        // try to write an error back to the client
                        let _ = tls.write_tls(&mut self.stream);
                        Err(Error::new(ErrorKind::Other, "tls error"))
                    }
                }
            }
        } else {
            self.buffer.read_from(&mut self.stream)
        }
    }

    /// Return true if there are still bytes in the tx buffer
    pub fn tx_pending(&self) -> bool {
        self.buffer.write_pending() > 0
    }

    /// Write to the session buffer
    pub fn write(&mut self, buf: &[u8]) -> Result<usize, std::io::Error> {
        self.buffer.write(buf)
    }

    /// Flush the session buffer to the stream
    pub fn flush(&mut self) -> Result<Option<usize>, std::io::Error> {
        if let Some(ref mut tls) = self.tls {
            self.buffer.write_to(tls)?;
            if tls.wants_write() {
                tls.write_tls(&mut self.stream).map(|v| Some(v))
            } else {
                Ok(None)
            }
        } else {
            self.buffer.write_to(&mut self.stream)
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
            if tls.wants_read() && !tls.wants_write() {
                match self.state {
                    State::Reading => Interest::READABLE,
                    _ => Interest::READABLE | Interest::WRITABLE,
                }
            } else if tls.wants_write() && !tls.wants_read() {
                match self.state {
                    State::Writing => Interest::WRITABLE,
                    _ => Interest::READABLE | Interest::WRITABLE,
                }
            } else {
                Interest::READABLE | Interest::WRITABLE
            }
        } else {
            match self.state {
                State::Reading => Interest::READABLE,
                State::Writing => Interest::WRITABLE,
                State::Handshaking => Interest::READABLE | Interest::WRITABLE,
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
        let _ = self.stream.shutdown(std::net::Shutdown::Both);
        self.tls = None;
    }
}

pub enum State {
    Handshaking,
    Reading,
    Writing,
}
