// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

pub use boring::ssl::{ShutdownResult, SslVerifyMode};
use std::os::unix::prelude::AsRawFd;

use boring::ssl::{ErrorCode, Ssl, SslFiletype, SslMethod, SslStream};
use boring::x509::X509;

use crate::*;

#[derive(PartialEq)]
enum TlsState {
    Handshaking,
    Negotiated,
}

/// Wraps a TLS/SSL stream so that negotiated and handshaking sessions have a
/// uniform type.
pub struct TlsTcpStream {
    inner: SslStream<TcpStream>,
    state: TlsState,
}

impl AsRawFd for TlsTcpStream {
    fn as_raw_fd(&self) -> i32 {
        self.inner.get_ref().as_raw_fd()
    }
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
                STREAM_HANDSHAKE.increment();
                self.state = TlsState::Negotiated;
                Ok(())
            } else {
                let code = unsafe { ErrorCode::from_raw(boring_sys::SSL_get_error(ptr, ret)) };
                match code {
                    ErrorCode::WANT_READ | ErrorCode::WANT_WRITE => {
                        Err(Error::from(ErrorKind::WouldBlock))
                    }
                    _ => {
                        STREAM_HANDSHAKE.increment();
                        STREAM_HANDSHAKE_EX.increment();
                        Err(Error::new(ErrorKind::Other, "handshake failed"))
                    }
                }
            }
        } else {
            Ok(())
        }
    }

    pub fn shutdown(&mut self) -> Result<ShutdownResult> {
        self.inner
            .shutdown()
            .map_err(|e| Error::new(ErrorKind::Other, e.to_string()))
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

/// Provides a wrapped acceptor for server-side TLS. This returns our wrapped
/// `TlsStream` type so that clients can store negotiated and handshaking
/// streams in a structure with a uniform type.
pub struct TlsTcpAcceptor {
    inner: boring::ssl::SslContext,
}

impl TlsTcpAcceptor {
    pub fn mozilla_intermediate_v5() -> Result<TlsTcpAcceptorBuilder> {
        let inner = boring::ssl::SslAcceptor::mozilla_intermediate_v5(SslMethod::tls_server())
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

/// Provides a wrapped connector for client-side TLS. This returns our wrapped
/// `TlsStream` type so that clients can store negotiated and handshaking
/// streams in a structure with a uniform type.
#[allow(dead_code)]
pub struct TlsTcpConnector {
    inner: boring::ssl::SslContext,
}

impl TlsTcpConnector {
    pub fn builder() -> Result<TlsTcpConnectorBuilder> {
        let inner = boring::ssl::SslConnector::builder(SslMethod::tls_client())
            .map_err(|e| Error::new(ErrorKind::Other, e.to_string()))?;

        Ok(TlsTcpConnectorBuilder {
            inner,
            ca_file: None,
            certificate_file: None,
            certificate_chain_file: None,
            private_key_file: None,
        })
    }

    pub fn connect<A: ToSocketAddrs>(&self, addr: A) -> Result<TlsTcpStream> {
        let addrs: Vec<SocketAddr> = addr.to_socket_addrs()?.collect();
        let mut s = Err(Error::new(ErrorKind::Other, "failed to resolve"));
        for addr in addrs {
            s = TcpStream::connect(addr);
            if s.is_ok() {
                break;
            }
        }

        let ssl = Ssl::new(&self.inner)?;

        let stream = unsafe { SslStream::from_raw_parts(ssl.into_ptr(), s?) };

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

/// Provides a wrapped builder for producing a `TlsConnector`. This has some
/// minor differences from the `boring::ssl::SslConnectorBuilder` to provide
/// improved ergonomics.
pub struct TlsTcpConnectorBuilder {
    inner: boring::ssl::SslConnectorBuilder,
    ca_file: Option<PathBuf>,
    certificate_file: Option<PathBuf>,
    certificate_chain_file: Option<PathBuf>,
    private_key_file: Option<PathBuf>,
}

impl TlsTcpConnectorBuilder {
    pub fn build(mut self) -> Result<TlsTcpConnector> {
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

        Ok(TlsTcpConnector { inner })
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

// NOTE: these tests only work if there's a `test` folder within this crate that
// contains the necessary keys and certs. They are left here for reference and
// in the future we should automate creation of self-signed keys and certs for
// use for testing during local development and in CI.

// #[cfg(test)]
// mod tests {
//     use super::*;

//     fn gen_keys() -> Result<(), ()> {

//     }

//     fn create_connector() -> Connector {
//         let tls_connector = TlsTcpConnector::builder()
//             .expect("failed to create builder")
//             .ca_file("test/root.crt")
//             .certificate_chain_file("test/client.crt")
//             .private_key_file("test/client.key")
//             .build()
//             .expect("failed to initialize tls connector");

//         Connector::from(tls_connector)
//     }

//     fn create_listener(addr: &'static str) -> Listener {
//         let tcp_listener = TcpListener::bind(addr).expect("failed to bind");
//         let tls_acceptor = TlsTcpAcceptor::mozilla_intermediate_v5()
//             .expect("failed to create builder")
//             .ca_file("test/root.crt")
//             .certificate_chain_file("test/server.crt")
//             .private_key_file("test/server.key")
//             .build()
//             .expect("failed to initialize tls acceptor");

//         Listener::from((tcp_listener, tls_acceptor))
//     }

//     #[test]
//     fn listener() {
//         let _ = create_listener("127.0.0.1:0");
//     }

//     #[test]
//     fn connector() {
//         let _ = create_connector();
//     }

//     #[test]
//     fn ping_pong() {
//         let connector = create_connector();
//         let listener = create_listener("127.0.0.1:0");

//         let addr = listener.local_addr().expect("listener has no local addr");

//         let mut client_stream = connector.connect(addr).expect("failed to connect");
//         std::thread::sleep(std::time::Duration::from_millis(100));
//         let mut server_stream = listener.accept().expect("failed to accept");

//         let mut server_handshake_complete = false;
//         let mut client_handshake_complete = false;

//         while !(server_handshake_complete && client_handshake_complete) {
//             if !server_handshake_complete {
//                 std::thread::sleep(std::time::Duration::from_millis(100));
//                 if server_stream.do_handshake().is_ok() {
//                     server_handshake_complete = true;
//                 }
//             }

//             if !client_handshake_complete {
//                 std::thread::sleep(std::time::Duration::from_millis(100));
//                 if client_stream.do_handshake().is_ok() {
//                     client_handshake_complete = true;
//                 }
//             }
//         }

//         std::thread::sleep(std::time::Duration::from_millis(100));

//         client_stream
//             .write_all(b"PING\r\n")
//             .expect("failed to write");
//         client_stream.flush().expect("failed to flush");

//         std::thread::sleep(std::time::Duration::from_millis(100));

//         let mut buf = [0; 4096];

//         match server_stream.read(&mut buf) {
//             Ok(6) => {
//                 assert_eq!(&buf[0..6], b"PING\r\n");
//                 server_stream
//                     .write_all(b"PONG\r\n")
//                     .expect("failed to write");
//             }
//             Ok(n) => {
//                 panic!("read: {} bytes but expected 6", n);
//             }
//             Err(e) => {
//                 panic!("error reading: {}", e);
//             }
//         }

//         std::thread::sleep(std::time::Duration::from_millis(100));

//         match client_stream.read(&mut buf) {
//             Ok(6) => {
//                 assert_eq!(&buf[0..6], b"PONG\r\n");
//             }
//             Ok(n) => {
//                 panic!("read: {} bytes but expected 6", n);
//             }
//             Err(e) => {
//                 panic!("error reading: {}", e);
//             }
//         }
//     }
// }
