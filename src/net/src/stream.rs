// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::*;

counter!(STREAM_ACCEPT);
counter!(STREAM_ACCEPT_EX);
counter!(STREAM_HANDSHAKE);
counter!(STREAM_HANDSHAKE_EX);

/// A wrapper type that unifies types which represent a stream. For example,
/// plaintext TCP streams and TLS/SSL over TCP can both be wrapped by this type.
/// This allows dynamic behaviors at runtime, such as enabling TLS/SSL through
/// configuration or allowing clients to request an upgrade to TLS/SSL from a
/// plaintext stream.
pub struct Stream {
    inner: StreamType,
}

impl Stream {
    pub fn interest(&self) -> Interest {
        match &self.inner {
            StreamType::Tcp(s) => {
                if !s.is_established() {
                    Interest::READABLE.add(Interest::WRITABLE)
                } else {
                    Interest::READABLE
                }
            }
            StreamType::TlsTcp(s) => s.interest(),
        }
    }

    pub fn is_established(&self) -> bool {
        match &self.inner {
            StreamType::Tcp(s) => s.is_established(),
            StreamType::TlsTcp(s) => !s.is_handshaking(),
        }
    }

    pub fn is_handshaking(&self) -> bool {
        match &self.inner {
            StreamType::Tcp(_) => false,
            StreamType::TlsTcp(s) => s.is_handshaking(),
        }
    }

    pub fn do_handshake(&mut self) -> Result<()> {
        match &mut self.inner {
            StreamType::Tcp(_) => Ok(()),
            StreamType::TlsTcp(s) => s.do_handshake(),
        }
    }

    pub fn set_nodelay(&mut self, nodelay: bool) -> Result<()> {
        match &mut self.inner {
            StreamType::Tcp(s) => s.set_nodelay(nodelay),
            StreamType::TlsTcp(s) => s.set_nodelay(nodelay),
        }
    }
}

impl Debug for Stream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        match &self.inner {
            StreamType::Tcp(s) => write!(f, "{:?}", s),
            StreamType::TlsTcp(s) => write!(f, "{:?}", s),
        }
    }
}

impl From<TcpStream> for Stream {
    fn from(other: TcpStream) -> Self {
        Self {
            inner: StreamType::Tcp(other),
        }
    }
}

impl From<TlsTcpStream> for Stream {
    fn from(other: TlsTcpStream) -> Self {
        Self {
            inner: StreamType::TlsTcp(other),
        }
    }
}

impl Read for Stream {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        match &mut self.inner {
            StreamType::Tcp(s) => s.read(buf),
            StreamType::TlsTcp(s) => s.read(buf),
        }
    }
}

impl Write for Stream {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        match &mut self.inner {
            StreamType::Tcp(s) => s.write(buf),
            StreamType::TlsTcp(s) => s.write(buf),
        }
    }

    fn flush(&mut self) -> Result<()> {
        match &mut self.inner {
            StreamType::Tcp(s) => s.flush(),
            StreamType::TlsTcp(s) => s.flush(),
        }
    }
}

impl event::Source for Stream {
    fn register(&mut self, registry: &Registry, token: Token, interest: Interest) -> Result<()> {
        match &mut self.inner {
            StreamType::Tcp(s) => s.register(registry, token, interest),
            StreamType::TlsTcp(s) => s.register(registry, token, interest),
        }
    }

    fn reregister(
        &mut self,
        registry: &mio::Registry,
        token: mio::Token,
        interest: mio::Interest,
    ) -> Result<()> {
        match &mut self.inner {
            StreamType::Tcp(s) => s.reregister(registry, token, interest),
            StreamType::TlsTcp(s) => s.reregister(registry, token, interest),
        }
    }

    fn deregister(&mut self, registry: &mio::Registry) -> Result<()> {
        match &mut self.inner {
            StreamType::Tcp(s) => s.deregister(registry),
            StreamType::TlsTcp(s) => s.deregister(registry),
        }
    }
}

/// Provides concrete types for stream variants. Since the number of variants is
/// expected to be small, dispatch through enum variants should be more
/// efficient than using a trait for dynamic dispatch.
enum StreamType {
    Tcp(TcpStream),
    TlsTcp(TlsTcpStream),
}
