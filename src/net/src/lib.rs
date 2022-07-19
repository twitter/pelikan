// use boring::ssl::{SslConnector, SslAcceptor};
use core::ops::Deref;
pub use mio::*;
use std::net::SocketAddr;

// use std::io::Read;
use std::io::ErrorKind;

use boring::ssl::HandshakeError;

mod tcp;

pub use tcp::*;

pub mod event {
    pub use mio::event::*;
}

use std::io::{Error, Read, Write};
use std::net::ToSocketAddrs;

type Result<T> = std::io::Result<T>;

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
            StreamType::Tcp(s) => s.peer_addr().is_ok(),
            StreamType::TlsTcp(s) => !s.is_handshaking(),
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

impl From<TlsStream<TcpStream>> for Stream {
    fn from(other: TlsStream<TcpStream>) -> Self {
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

/// Provides concrete types for stream variants. Since the number of variants is
/// expected to be small, dispatch through enum variants should be more
/// efficient than using a trait for dynamic dispatch.
enum StreamType {
    Tcp(TcpStream),
    TlsTcp(TlsStream<TcpStream>),
}

/// Wraps a TLS/SSL stream so that negotiated and handshaking sessions have a
/// uniform type.
pub struct TlsStream<S> {
    inner: TlsState<S>,
}

impl<S> TlsStream<S>
where
    S: Read + Write,
{
    pub fn is_handshaking(&self) -> bool {
        match self.inner {
            TlsState::Negotiated(_) => false,
            TlsState::Handshaking(_) => true,
        }
    }

    pub fn interest(&self) -> Interest {
        if self.is_handshaking() {
            Interest::READABLE.add(Interest::WRITABLE)
        } else {
            Interest::READABLE
        }
    }
}

impl<S> Read for TlsStream<S>
where
    S: Read + Write,
{
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        match &mut self.inner {
            TlsState::Negotiated(s) => s.read(buf),
            TlsState::Handshaking(_) => Err(Error::new(
                ErrorKind::WouldBlock,
                "read on handshaking session would block",
            )),
        }
    }
}

impl<S> Write for TlsStream<S>
where
    S: Read + Write,
{
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        match &mut self.inner {
            TlsState::Negotiated(s) => s.write(buf),
            TlsState::Handshaking(_) => Err(Error::new(
                ErrorKind::WouldBlock,
                "write on handshaking session would block",
            )),
        }
    }

    fn flush(&mut self) -> Result<()> {
        match &mut self.inner {
            TlsState::Negotiated(s) => s.flush(),
            TlsState::Handshaking(_) => Err(Error::new(
                ErrorKind::WouldBlock,
                "flush on handshaking session would block",
            )),
        }
    }
}

/// Provides a wrapped connector for client-side TLS. This returns our wrapped
/// `TlsStream` type so that clients can store negotiated and handshaking
/// streams in a structure with a uniform type.
pub struct TlsConnector {
    inner: boring::ssl::SslConnector,
}

impl TlsConnector {
    pub fn connect<S: Read + Write>(&self, domain: &str, stream: S) -> Result<TlsStream<S>> {
        let state = match self.inner.connect(domain, stream) {
            Ok(s) => TlsState::Negotiated(s),
            Err(e) => match e {
                HandshakeError::SetupFailure(e) => {
                    return Err(Error::new(ErrorKind::Other, e.to_string()));
                }
                HandshakeError::Failure(_) => {
                    return Err(Error::new(ErrorKind::Other, "ssl handshake failure"));
                }
                HandshakeError::WouldBlock(s) => TlsState::Handshaking(s),
            },
        };

        Ok(TlsStream { inner: state })
    }
}

/// Polymorphism via enum to allow both negotiated and handshaking TLS/SSL
/// streams to be represented by a single type.
enum TlsState<T> {
    Handshaking(boring::ssl::MidHandshakeSslStream<T>),
    Negotiated(boring::ssl::SslStream<T>),
}
