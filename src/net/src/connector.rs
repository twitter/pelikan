// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::*;

pub struct Connector {
    inner: ConnectorType,
}

enum ConnectorType {
    Tcp(TcpConnector),
    TlsTcp(TlsTcpConnector),
}

impl Connector {
    /// Attemps to connect to the provided address.
    pub fn connect<A: ToSocketAddrs>(&self, addr: A) -> Result<Stream> {
        match &self.inner {
            ConnectorType::Tcp(connector) => Ok(Stream::from(connector.connect(addr)?)),
            ConnectorType::TlsTcp(connector) => Ok(Stream::from(connector.connect(addr)?)),
        }
    }
}

impl From<TcpConnector> for Connector {
    fn from(other: TcpConnector) -> Self {
        Self {
            inner: ConnectorType::Tcp(other),
        }
    }
}

impl From<TlsTcpConnector> for Connector {
    fn from(other: TlsTcpConnector) -> Self {
        Self {
            inner: ConnectorType::TlsTcp(other),
        }
    }
}
