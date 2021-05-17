//! Encapsulates plaintext and TLS sessions into a single type.
use super::TcpStream;
use boring::ssl::HandshakeError;
use boring::ssl::MidHandshakeSslStream;
use boring::ssl::SslStream;
use metrics::Stat;
use std::io::{Error, ErrorKind};
use std::io::{Read, Write};

pub struct Stream {
    inner: Option<StreamType>,
}

pub enum StreamType {
    /// An established plaintext TCP connection
    Plain(TcpStream),
    /// A TLS/SSL TCP stream which is fully negotiated
    Tls(SslStream<TcpStream>),
    /// A TLS/SSL TCP stream which is still handshaking
    Handshaking(MidHandshakeSslStream<TcpStream>),
}

impl Stream {
    pub fn plain(tcp_stream: TcpStream) -> Self {
        Self {
            inner: Some(StreamType::Plain(tcp_stream)),
        }
    }

    pub fn tls(ssl_stream: SslStream<TcpStream>) -> Self {
        Self {
            inner: Some(StreamType::Tls(ssl_stream)),
        }
    }

    pub fn handshaking(handshaking_ssl_stream: MidHandshakeSslStream<TcpStream>) -> Self {
        Self {
            inner: Some(StreamType::Handshaking(handshaking_ssl_stream)),
        }
    }

    pub fn is_handshaking(&self) -> bool {
        matches!(self.inner, Some(StreamType::Handshaking(_)))
    }

    pub fn do_handshake(&mut self) -> Result<(), std::io::Error> {
        if let Some(StreamType::Handshaking(stream)) = self.inner.take() {
            let ret;
            let result = stream.handshake();
            self.inner = match result {
                Ok(established) => {
                    ret = Ok(());
                    Some(StreamType::Tls(established))
                }
                Err(HandshakeError::WouldBlock(handshaking)) => {
                    ret = Err(Error::new(ErrorKind::WouldBlock, "handshake would block"));
                    Some(StreamType::Handshaking(handshaking))
                }
                _ => {
                    ret = Err(Error::new(ErrorKind::Other, "handshaking error"));
                    None
                }
            };
            ret
        } else {
            Err(Error::new(
                ErrorKind::Other,
                "session is not in handshaking state",
            ))
        }
    }

    pub fn close(&mut self) {
        increment_counter!(&Stat::TcpClose);
        if let Some(stream) = self.inner.take() {
            self.inner = match stream {
                StreamType::Plain(s) => {
                    let _ = s.shutdown(std::net::Shutdown::Both);
                    Some(StreamType::Plain(s))
                }
                StreamType::Tls(mut s) => {
                    // TODO(bmartin): session resume requires that a full graceful
                    // shutdown occurs
                    let _ = s.shutdown();
                    Some(StreamType::Tls(s))
                }
                StreamType::Handshaking(mut s) => {
                    // since we don't have a fully established session, just
                    // shutdown the underlying tcp stream
                    let _ = s.get_mut().shutdown(std::net::Shutdown::Both);
                    Some(StreamType::Handshaking(s))
                }
            }
        }
    }
}

impl Read for Stream {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
        if let Some(stream) = &mut self.inner {
            stream.read(buf)
        } else {
            Err(Error::new(ErrorKind::Other, "stream is disconnected"))
        }
    }
}

impl Read for StreamType {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
        match self {
            Self::Plain(s) => s.read(buf),
            Self::Tls(s) => s.read(buf),
            Self::Handshaking(_) => Err(Error::new(
                ErrorKind::WouldBlock,
                "handshaking tls stream would block on read",
            )),
        }
    }
}

impl Write for Stream {
    fn write(&mut self, buf: &[u8]) -> Result<usize, Error> {
        if let Some(stream) = &mut self.inner {
            stream.write(buf)
        } else {
            Err(Error::new(ErrorKind::Other, "stream is disconnected"))
        }
    }
    fn flush(&mut self) -> Result<(), std::io::Error> {
        if let Some(stream) = &mut self.inner {
            stream.flush()
        } else {
            Ok(())
        }
    }
}

impl Write for StreamType {
    fn write(&mut self, buf: &[u8]) -> Result<usize, Error> {
        match self {
            Self::Plain(s) => s.write(buf),
            Self::Tls(s) => s.write(buf),
            Self::Handshaking(_) => Err(Error::new(
                ErrorKind::WouldBlock,
                "handshaking tls stream would block on write",
            )),
        }
    }

    fn flush(&mut self) -> std::result::Result<(), std::io::Error> {
        Ok(())
    }
}

impl mio::event::Source for Stream {
    fn register(
        &mut self,
        registry: &mio::Registry,
        token: mio::Token,
        interest: mio::Interest,
    ) -> std::result::Result<(), std::io::Error> {
        if let Some(stream) = &mut self.inner {
            stream.register(registry, token, interest)
        } else {
            Err(Error::new(ErrorKind::Other, "stream is disconnected"))
        }
    }

    fn reregister(
        &mut self,
        registry: &mio::Registry,
        token: mio::Token,
        interest: mio::Interest,
    ) -> std::result::Result<(), std::io::Error> {
        if let Some(stream) = &mut self.inner {
            stream.reregister(registry, token, interest)
        } else {
            Err(Error::new(ErrorKind::Other, "stream is disconnected"))
        }
    }

    fn deregister(&mut self, registry: &mio::Registry) -> std::result::Result<(), std::io::Error> {
        if let Some(stream) = &mut self.inner {
            stream.deregister(registry)
        } else {
            Err(Error::new(ErrorKind::Other, "stream is disconnected"))
        }
    }
}

impl mio::event::Source for StreamType {
    fn register(
        &mut self,
        registry: &mio::Registry,
        token: mio::Token,
        interest: mio::Interest,
    ) -> std::result::Result<(), std::io::Error> {
        match self {
            Self::Plain(s) => registry.register(s, token, interest),
            Self::Tls(s) => registry.register(s.get_mut(), token, interest),
            Self::Handshaking(s) => registry.register(s.get_mut(), token, interest),
        }
    }

    fn reregister(
        &mut self,
        registry: &mio::Registry,
        token: mio::Token,
        interest: mio::Interest,
    ) -> std::result::Result<(), std::io::Error> {
        match self {
            Self::Plain(s) => registry.reregister(s, token, interest),
            Self::Tls(s) => registry.reregister(s.get_mut(), token, interest),
            Self::Handshaking(s) => registry.reregister(s.get_mut(), token, interest),
        }
    }

    fn deregister(&mut self, registry: &mio::Registry) -> std::result::Result<(), std::io::Error> {
        match self {
            Self::Plain(s) => registry.deregister(s),
            Self::Tls(s) => registry.deregister(s.get_mut()),
            Self::Handshaking(s) => registry.deregister(s.get_mut()),
        }
    }
}