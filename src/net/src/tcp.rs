// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::*;
use std::os::unix::prelude::FromRawFd;

pub use std::net::Shutdown;

#[derive(PartialEq)]
enum State {
    Connecting,
    Established,
}

pub struct TcpStream {
    inner: mio::net::TcpStream,
    state: State,
}

impl TcpStream {
    pub fn connect(addr: SocketAddr) -> Result<Self> {
        let inner = mio::net::TcpStream::connect(addr)?;

        TCP_CONN_CURR.increment();
        TCP_CONNECT.increment();

        Ok(Self {
            inner,
            state: State::Connecting,
        })
    }

    pub fn is_established(&mut self) -> bool {
        if self.state == State::Established {
            true
        } else if self.inner.peer_addr().is_ok() {
            self.state = State::Established;
            true
        } else {
            false
        }
    }

    pub fn from_std(stream: std::net::TcpStream) -> Self {
        let inner = mio::net::TcpStream::from_std(stream);
        let state = if inner.peer_addr().is_ok() {
            State::Established
        } else {
            State::Connecting
        };

        Self { inner, state }
    }

    pub fn set_nodelay(&mut self, nodelay: bool) -> Result<()> {
        self.inner.set_nodelay(nodelay)
    }
}

impl Drop for TcpStream {
    fn drop(&mut self) {
        TCP_CONN_CURR.decrement();
        TCP_CLOSE.increment();
    }
}

impl Debug for TcpStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        write!(f, "{:?}", self.inner)
    }
}

impl Deref for TcpStream {
    type Target = mio::net::TcpStream;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl Read for TcpStream {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        match self.inner.read(buf) {
            Ok(amt) => {
                TCP_RECV_BYTE.add(amt as _);
                Ok(amt)
            }
            Err(e) => Err(e),
        }
    }
}

impl Write for TcpStream {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        match self.inner.write(buf) {
            Ok(amt) => {
                TCP_SEND_BYTE.add(amt as _);
                Ok(amt)
            }
            Err(e) => Err(e),
        }
    }

    fn flush(&mut self) -> Result<()> {
        self.inner.flush()
    }
}

impl event::Source for TcpStream {
    fn register(
        &mut self,
        registry: &mio::Registry,
        token: mio::Token,
        interest: mio::Interest,
    ) -> Result<()> {
        self.inner.register(registry, token, interest)
    }

    fn reregister(
        &mut self,
        registry: &mio::Registry,
        token: mio::Token,
        interest: mio::Interest,
    ) -> Result<()> {
        self.inner.reregister(registry, token, interest)
    }

    fn deregister(&mut self, registry: &mio::Registry) -> Result<()> {
        self.inner.deregister(registry)
    }
}

impl FromRawFd for TcpStream {
    unsafe fn from_raw_fd(raw_fd: i32) -> Self {
        let inner = mio::net::TcpStream::from_raw_fd(raw_fd);
        let state = if inner.peer_addr().is_ok() {
            State::Established
        } else {
            State::Connecting
        };

        Self { inner, state }
    }
}

pub struct TcpListener {
    inner: mio::net::TcpListener,
}

impl Deref for TcpListener {
    type Target = mio::net::TcpListener;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl TcpListener {
    pub fn bind<A: ToSocketAddrs>(addr: A) -> Result<TcpListener> {
        // we create from a std TcpListener so SO_REUSEADDR is not set for us
        let l = std::net::TcpListener::bind(addr)?;
        // this means we need to set non-blocking ourselves
        l.set_nonblocking(true)?;

        let inner = mio::net::TcpListener::from_std(l);

        Ok(Self { inner })
    }

    pub fn accept(&self) -> Result<(TcpStream, SocketAddr)> {
        let result = self.inner.accept().map(|(stream, addr)| {
            (
                TcpStream {
                    inner: stream,
                    state: State::Established,
                },
                addr,
            )
        });

        if result.is_ok() {
            TCP_ACCEPT.increment();
            TCP_CONN_CURR.increment();
        }

        result
    }

    pub fn local_addr(&self) -> Result<SocketAddr> {
        self.inner.local_addr()
    }
}

impl event::Source for TcpListener {
    fn register(
        &mut self,
        registry: &mio::Registry,
        token: mio::Token,
        interests: mio::Interest,
    ) -> Result<()> {
        self.inner.register(registry, token, interests)
    }

    fn reregister(
        &mut self,
        registry: &mio::Registry,
        token: mio::Token,
        interests: mio::Interest,
    ) -> Result<()> {
        self.inner.reregister(registry, token, interests)
    }

    fn deregister(&mut self, registry: &mio::Registry) -> Result<()> {
        self.inner.deregister(registry)
    }
}

#[derive(Default)]
pub struct TcpConnector {
    _inner: (),
}

impl TcpConnector {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn connect<A: ToSocketAddrs>(&self, addr: A) -> Result<TcpStream> {
        let addrs: Vec<SocketAddr> = addr.to_socket_addrs()?.collect();
        let mut s = Err(Error::new(ErrorKind::Other, "failed to resolve"));
        for addr in addrs {
            s = TcpStream::connect(addr);
            if s.is_ok() {
                break;
            }
        }

        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_connector() -> Connector {
        let tls_connector = TcpConnector::new();

        Connector::from(tls_connector)
    }

    fn create_listener(addr: &'static str) -> Listener {
        let tcp_listener = TcpListener::bind(addr).expect("failed to bind");

        Listener::from(tcp_listener)
    }

    #[test]
    fn listener() {
        let _ = create_listener("127.0.0.1:0");
    }

    #[test]
    fn connector() {
        let _ = create_connector();
    }

    #[test]
    fn ping_pong() {
        let connector = create_connector();
        let listener = create_listener("127.0.0.1:0");

        let addr = listener.local_addr().expect("listener has no local addr");

        let mut client_stream = connector.connect(addr).expect("failed to connect");
        std::thread::sleep(std::time::Duration::from_millis(100));
        let mut server_stream = listener.accept().expect("failed to accept");

        std::thread::sleep(std::time::Duration::from_millis(100));

        client_stream
            .write_all(b"PING\r\n")
            .expect("failed to write");
        client_stream.flush().expect("failed to flush");

        std::thread::sleep(std::time::Duration::from_millis(100));

        let mut buf = [0; 4096];

        match server_stream.read(&mut buf) {
            Ok(6) => {
                assert_eq!(&buf[0..6], b"PING\r\n");
                server_stream
                    .write_all(b"PONG\r\n")
                    .expect("failed to write");
            }
            Ok(n) => {
                panic!("read: {} bytes but expected 6", n);
            }
            Err(e) => {
                panic!("error reading: {}", e);
            }
        }

        std::thread::sleep(std::time::Duration::from_millis(100));

        match client_stream.read(&mut buf) {
            Ok(6) => {
                assert_eq!(&buf[0..6], b"PONG\r\n");
            }
            Ok(n) => {
                panic!("read: {} bytes but expected 6", n);
            }
            Err(e) => {
                panic!("error reading: {}", e);
            }
        }
    }
}
