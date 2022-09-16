// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::*;

pub struct Listener {
    inner: ListenerType,
}

enum ListenerType {
    Plain(TcpListener),
    Tls((TcpListener, TlsTcpAcceptor)),
}

impl From<TcpListener> for Listener {
    fn from(other: TcpListener) -> Self {
        Self {
            inner: ListenerType::Plain(other),
        }
    }
}

impl From<(TcpListener, TlsTcpAcceptor)> for Listener {
    fn from(other: (TcpListener, TlsTcpAcceptor)) -> Self {
        Self {
            inner: ListenerType::Tls(other),
        }
    }
}

impl Listener {
    /// Accepts a new `Stream`.
    ///
    /// An error `e` with `e.kind()` of `ErrorKind::WouldBlock` indicates that
    /// the operation should be retried again in the future.
    ///
    /// All other errors should be treated as failures.
    pub fn accept(&self) -> Result<Stream> {
        STREAM_ACCEPT.increment();
        let result = self._accept();
        if result.is_err() {
            STREAM_ACCEPT_EX.increment();
        }
        result
    }

    fn _accept(&self) -> Result<Stream> {
        match &self.inner {
            ListenerType::Plain(listener) => {
                let (stream, _addr) = listener.accept()?;
                Ok(Stream::from(stream))
            }
            ListenerType::Tls((listener, acceptor)) => {
                let (stream, _addr) = listener.accept()?;
                let stream = acceptor.accept(stream)?;
                Ok(Stream::from(stream))
            }
        }
    }

    pub fn local_addr(&self) -> Result<SocketAddr> {
        match &self.inner {
            ListenerType::Plain(listener) => listener.local_addr(),
            ListenerType::Tls((listener, _acceptor)) => listener.local_addr(),
        }
    }
}

impl event::Source for Listener {
    fn register(
        &mut self,
        registry: &mio::Registry,
        token: mio::Token,
        interests: mio::Interest,
    ) -> Result<()> {
        match &mut self.inner {
            ListenerType::Plain(listener) => listener.register(registry, token, interests),
            ListenerType::Tls((listener, _acceptor)) => {
                listener.register(registry, token, interests)
            }
        }
    }

    fn reregister(
        &mut self,
        registry: &mio::Registry,
        token: mio::Token,
        interests: mio::Interest,
    ) -> Result<()> {
        match &mut self.inner {
            ListenerType::Plain(listener) => listener.reregister(registry, token, interests),
            ListenerType::Tls((listener, _acceptor)) => {
                listener.reregister(registry, token, interests)
            }
        }
    }

    fn deregister(&mut self, registry: &mio::Registry) -> Result<()> {
        match &mut self.inner {
            ListenerType::Plain(listener) => listener.deregister(registry),
            ListenerType::Tls((listener, _acceptor)) => listener.deregister(registry),
        }
    }
}
