// use boring::ssl::{SslConnector, SslAcceptor};
use boring::ssl::ErrorCode;
use boring::ssl::Ssl;
use boring::ssl::SslStream;
pub use boring::ssl::{SslFiletype, SslMethod, SslVerifyMode};
use boring::x509::X509;
use core::fmt::Debug;
use core::ops::Deref;
use foreign_types_shared::ForeignType;
use foreign_types_shared::ForeignTypeRef;
pub use mio::*;
use std::io::ErrorKind;
use std::net::SocketAddr;
use std::path::Path;
use std::path::PathBuf;

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

/// Wraps a TLS/SSL stream so that negotiated and handshaking sessions have a
/// uniform type.
pub struct TlsTcpStream {
    inner: SslStream<TcpStream>,
    state: TlsState,
}

impl TlsTcpStream {
    pub fn set_nodelay(&mut self, nodelay: bool) -> Result<()> {
        self.inner.get_mut().set_nodelay(nodelay)
    }

    pub fn is_handshaking(&self) -> bool {
        self.state == TlsState::Handshaking
    }

    pub fn interest(&self) -> Interest {
        if self.is_handshaking() {
            Interest::READABLE.add(Interest::WRITABLE)
        } else {
            Interest::READABLE
        }
    }

    /// Attempts to drive the TLS/SSL handshake to completion. If the return
    /// variant is `Ok` it indiates that the handshake is complete. An error
    /// result of `WouldBlock` indicates that the handshake may complete in the
    /// future. Other error types indiate a handshake failure with no possible
    /// recovery and that the connection should be closed.
    pub fn do_handshake(&mut self) -> Result<()> {
        if self.is_handshaking() {
            let ptr = self.inner.ssl().as_ptr();
            let ret = unsafe { boring_sys::SSL_do_handshake(ptr) };
            if ret > 0 {
                self.state = TlsState::Negotiated;
                Ok(())
            } else {
                let code = unsafe { ErrorCode::from_raw(boring_sys::SSL_get_error(ptr, ret)) };
                match code {
                    ErrorCode::WANT_READ | ErrorCode::WANT_WRITE => {
                        Err(Error::from(ErrorKind::WouldBlock))
                    }
                    _ => Err(Error::new(ErrorKind::Other, "handshake failed")),
                }
            }
        } else {
            Ok(())
        }
    }
}

impl Debug for TlsTcpStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        write!(f, "{:?}", self.inner.get_ref())
    }
}

impl Read for TlsTcpStream {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        if self.is_handshaking() {
            Err(Error::new(
                ErrorKind::WouldBlock,
                "read on handshaking session would block",
            ))
        } else {
            self.inner.read(buf)
        }
    }
}

impl Write for TlsTcpStream {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        if self.is_handshaking() {
            Err(Error::new(
                ErrorKind::WouldBlock,
                "write on handshaking session would block",
            ))
        } else {
            self.inner.write(buf)
        }
    }

    fn flush(&mut self) -> Result<()> {
        if self.is_handshaking() {
            Err(Error::new(
                ErrorKind::WouldBlock,
                "flush on handshaking session would block",
            ))
        } else {
            self.inner.flush()
        }
    }
}

impl event::Source for TlsTcpStream {
    fn register(&mut self, registry: &Registry, token: Token, interest: Interest) -> Result<()> {
        self.inner.get_mut().register(registry, token, interest)
    }

    fn reregister(
        &mut self,
        registry: &mio::Registry,
        token: mio::Token,
        interest: mio::Interest,
    ) -> Result<()> {
        self.inner.get_mut().reregister(registry, token, interest)
    }

    fn deregister(&mut self, registry: &mio::Registry) -> Result<()> {
        self.inner.get_mut().deregister(registry)
    }
}

pub struct Connector {
    inner: ConnectorType,
}

enum ConnectorType {
    Plain,
    Tls(TlsTcpConnector),
}

impl Connector {
    pub fn connect<A: ToSocketAddrs>(&self, addr: A) -> Result<Stream> {
        match &self.inner {
            ConnectorType::Plain => {
                let addrs: Vec<SocketAddr> = addr.to_socket_addrs()?.collect();
                let mut s = Err(Error::new(ErrorKind::Other, "failed to resolve"));
                for addr in addrs {
                    s = TcpStream::connect(addr);
                    if s.is_ok() {
                        break;
                    }
                }
                Ok(Stream::from(s?))
            }
            ConnectorType::Tls(_connector) => {
                todo!()
            }
        }
    }
}

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
    pub fn accept(&self) -> Result<Stream> {
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

/// Provides a wrapped connector for client-side TLS. This returns our wrapped
/// `TlsStream` type so that clients can store negotiated and handshaking
/// streams in a structure with a uniform type.
pub struct TlsTcpConnector {
    inner: boring::ssl::SslContext,
}

impl TlsTcpConnector {
    pub fn connect(&self, domain: &str, stream: TcpStream) -> Result<TlsTcpStream> {
        let mut ssl = Ssl::new(&self.inner)?;

        // set hostname for SNI
        ssl.set_hostname(domain)?;

        // verify hostname
        let param = ssl.param_mut();
        param.set_hostflags(boring::x509::verify::X509CheckFlags::NO_PARTIAL_WILDCARDS);
        match domain.parse() {
            Ok(ip) => param.set_ip(ip),
            Err(_) => param.set_host(domain),
        }?;

        let stream = unsafe { SslStream::from_raw_parts(ssl.into_ptr(), stream) };

        let ret = unsafe { boring_sys::SSL_connect(stream.ssl().as_ptr()) };

        if ret > 0 {
            Ok(TlsTcpStream {
                inner: stream,
                state: TlsState::Negotiated,
            })
        } else {
            let code = unsafe {
                ErrorCode::from_raw(boring_sys::SSL_get_error(stream.ssl().as_ptr(), ret))
            };
            match code {
                ErrorCode::WANT_READ | ErrorCode::WANT_WRITE => Ok(TlsTcpStream {
                    inner: stream,
                    state: TlsState::Handshaking,
                }),
                _ => Err(Error::new(ErrorKind::Other, "handshake failed")),
            }
        }
    }
}

#[derive(PartialEq)]
enum TlsState {
    Handshaking,
    Negotiated,
}

/// Provides a wrapped acceptor for server-side TLS. This returns our wrapped
/// `TlsStream` type so that clients can store negotiated and handshaking
/// streams in a structure with a uniform type.
pub struct TlsTcpAcceptor {
    inner: boring::ssl::SslContext,
}

impl TlsTcpAcceptor {
    pub fn mozilla_intermediate_v5(method: SslMethod) -> Result<TlsTcpAcceptorBuilder> {
        let inner = boring::ssl::SslAcceptor::mozilla_intermediate_v5(method)
            .map_err(|e| Error::new(ErrorKind::Other, e.to_string()))?;

        Ok(TlsTcpAcceptorBuilder {
            inner,
            ca_file: None,
            certificate_file: None,
            certificate_chain_file: None,
            private_key_file: None,
        })
    }

    pub fn accept(&self, stream: TcpStream) -> Result<TlsTcpStream> {
        let ssl = Ssl::new(&self.inner)?;

        let stream = unsafe { SslStream::from_raw_parts(ssl.into_ptr(), stream) };

        let ret = unsafe { boring_sys::SSL_accept(stream.ssl().as_ptr()) };

        if ret > 0 {
            Ok(TlsTcpStream {
                inner: stream,
                state: TlsState::Negotiated,
            })
        } else {
            let code = unsafe {
                ErrorCode::from_raw(boring_sys::SSL_get_error(stream.ssl().as_ptr(), ret))
            };
            match code {
                ErrorCode::WANT_READ | ErrorCode::WANT_WRITE => Ok(TlsTcpStream {
                    inner: stream,
                    state: TlsState::Handshaking,
                }),
                _ => Err(Error::new(ErrorKind::Other, "handshake failed")),
            }
        }
    }
}

/// Provides a wrapped builder for producing a `TlsAcceptor`. This has some
/// minor differences from the `boring::ssl::SslAcceptorBuilder` to provide
/// improved ergonomics.
pub struct TlsTcpAcceptorBuilder {
    inner: boring::ssl::SslAcceptorBuilder,
    ca_file: Option<PathBuf>,
    certificate_file: Option<PathBuf>,
    certificate_chain_file: Option<PathBuf>,
    private_key_file: Option<PathBuf>,
}

impl TlsTcpAcceptorBuilder {
    pub fn build(mut self) -> Result<TlsTcpAcceptor> {
        // load the CA file, if provided
        if let Some(f) = self.ca_file {
            self.inner.set_ca_file(f).map_err(|e| {
                Error::new(ErrorKind::Other, format!("failed to load CA file: {}", e))
            })?;
        }

        // load the private key from file
        if let Some(f) = self.private_key_file {
            self.inner
                .set_private_key_file(f, SslFiletype::PEM)
                .map_err(|e| {
                    Error::new(
                        ErrorKind::Other,
                        format!("failed to load private key file: {}", e),
                    )
                })?;
        } else {
            return Err(Error::new(ErrorKind::Other, "no private key file provided"));
        }

        // load the certificate chain, certificate file, or both
        match (self.certificate_chain_file, self.certificate_file) {
            (Some(chain), Some(cert)) => {
                // assume we have the leaf in a standalone file, and the
                // intermediates + root in another file

                // first load the leaf
                self.inner
                    .set_certificate_file(cert, SslFiletype::PEM)
                    .map_err(|e| {
                        Error::new(
                            ErrorKind::Other,
                            format!("failed to load certificate file: {}", e),
                        )
                    })?;

                // append the rest of the chain
                let pem = std::fs::read(chain).map_err(|e| {
                    Error::new(
                        ErrorKind::Other,
                        format!("failed to load certificate chain file: {}", e),
                    )
                })?;
                let chain = X509::stack_from_pem(&pem).map_err(|e| {
                    Error::new(
                        ErrorKind::Other,
                        format!("failed to load certificate chain file: {}", e),
                    )
                })?;
                for cert in chain {
                    self.inner.add_extra_chain_cert(cert).map_err(|e| {
                        Error::new(
                            ErrorKind::Other,
                            format!("bad certificate in certificate chain file: {}", e),
                        )
                    })?;
                }
            }
            (Some(chain), None) => {
                // assume we have a complete chain: leaf + intermediates + root in
                // one file

                // load the entire chain
                self.inner.set_certificate_chain_file(chain).map_err(|e| {
                    Error::new(
                        ErrorKind::Other,
                        format!("failed to load certificate chain file: {}", e),
                    )
                })?;
            }
            (None, Some(cert)) => {
                // this will just load the leaf certificate from the file
                self.inner
                    .set_certificate_file(cert, SslFiletype::PEM)
                    .map_err(|e| {
                        Error::new(
                            ErrorKind::Other,
                            format!("failed to load certificate file: {}", e),
                        )
                    })?;
            }
            (None, None) => {
                return Err(Error::new(
                    ErrorKind::Other,
                    "no certificate file or certificate chain file provided",
                ));
            }
        }

        let inner = self.inner.build().into_context();

        Ok(TlsTcpAcceptor { inner })
    }

    pub fn verify(mut self, mode: SslVerifyMode) -> Self {
        self.inner.set_verify(mode);
        self
    }

    /// Load trusted root certificates from a file.
    ///
    /// The file should contain a sequence of PEM-formatted CA certificates.
    pub fn ca_file<P: AsRef<Path>>(mut self, file: P) -> Self {
        self.ca_file = Some(file.as_ref().to_path_buf());
        self
    }

    /// Load a leaf certificate from a file.
    ///
    /// This loads only a single PEM-formatted certificate from the file which
    /// will be used as the leaf certifcate.
    ///
    /// Use `set_certificate_chain_file` to provide a complete certificate
    /// chain. Use this with the `set_certifcate_chain_file` if the leaf
    /// certifcate and remainder of the certificate chain are split across two
    /// files.
    pub fn certificate_file<P: AsRef<Path>>(mut self, file: P) -> Self {
        self.certificate_file = Some(file.as_ref().to_path_buf());
        self
    }

    /// Load a certificate chain from a file.
    ///
    /// The file should contain a sequence of PEM-formatted certificates. If
    /// used without `set_certificate_file` the provided file must contain the
    /// leaf certificate and the complete chain of certificates up to and
    /// including the trusted root certificate. If used with
    /// `set_certificate_file`, this file must not contain the leaf certifcate
    /// and will be treated as the complete chain of certificates up to and
    /// including the trusted root certificate.
    pub fn certificate_chain_file<P: AsRef<Path>>(mut self, file: P) -> Self {
        self.certificate_chain_file = Some(file.as_ref().to_path_buf());
        self
    }

    /// Loads the private key from a PEM-formatted file.
    pub fn private_key_file<P: AsRef<Path>>(mut self, file: P) -> Self {
        self.private_key_file = Some(file.as_ref().to_path_buf());
        self
    }
}
