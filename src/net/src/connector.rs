// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::*;

pub struct Connector {
    inner: ConnectorType,
}

enum ConnectorType {
    Tcp,
    TlsTcp(TlsTcpConnector),
}

impl Connector {
    /// Returns a new TCP `Connector`
    pub fn tcp() -> Self {
        Self {
            inner: ConnectorType::Tcp,
        }
    }

    /// Attemps to connect to the provided address.
    pub fn connect<A: ToSocketAddrs>(&self, addr: A) -> Result<Stream> {
        match &self.inner {
            ConnectorType::Tcp => {
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
            ConnectorType::TlsTcp(_connector) => {
                todo!()
            }
        }
    }
}

impl From<TlsTcpConnector> for Connector {
    fn from(other: TlsTcpConnector) -> Self {
        Self {
            inner: ConnectorType::TlsTcp(other)
        }
    }
}