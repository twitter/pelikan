// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! A new type wrapper for `TcpStream`s which allows for capturing metrics about
//! operations on the underlying TCP stream.

use std::convert::TryFrom;
use std::io::{Read, Write};
use std::net::SocketAddr;

use crate::{TCP_RECV_BYTE, TCP_SEND_BYTE, TCP_SEND_PARTIAL};

pub struct TcpStream {
    inner: mio::net::TcpStream,
}

impl TcpStream {
    pub fn shutdown(&self, how: std::net::Shutdown) -> Result<(), std::io::Error> {
        self.inner.shutdown(how)
    }

    pub fn peer_addr(&self) -> Result<SocketAddr, std::io::Error> {
        self.inner.peer_addr()
    }
}

impl TryFrom<mio::net::TcpStream> for TcpStream {
    type Error = std::io::Error;

    fn try_from(other: mio::net::TcpStream) -> Result<Self, std::io::Error> {
        let _ = other.peer_addr()?;
        Ok(Self { inner: other })
    }
}

impl Read for TcpStream {
    fn read(&mut self, buf: &mut [u8]) -> std::result::Result<usize, std::io::Error> {
        let result = self.inner.read(buf);
        if let Ok(bytes) = result {
            TCP_RECV_BYTE.add(bytes as _);
        }
        result
    }
}

impl Write for TcpStream {
    fn write(&mut self, buf: &[u8]) -> std::result::Result<usize, std::io::Error> {
        let result = self.inner.write(buf);
        if let Ok(bytes) = result {
            if bytes != buf.len() {
                TCP_SEND_PARTIAL.increment();
            }
            TCP_SEND_BYTE.add(bytes as _);
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
