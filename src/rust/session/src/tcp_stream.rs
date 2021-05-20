// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! A new type wrapper for `TcpStream`s which allows for capturing metrics about
//! operations on the underlying TCP stream.

use std::io::{Read, Write};

use metrics::Stat;

pub struct TcpStream {
    inner: mio::net::TcpStream,
}

impl TcpStream {
    pub fn shutdown(&self, how: std::net::Shutdown) -> Result<(), std::io::Error> {
        self.inner.shutdown(how)
    }
}

impl From<mio::net::TcpStream> for TcpStream {
    fn from(other: mio::net::TcpStream) -> Self {
        Self { inner: other }
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
            if bytes != buf.len() {
                increment_counter!(&Stat::TcpSendPartial);
            }
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
