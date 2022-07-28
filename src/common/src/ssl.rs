// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

pub use boring::ssl::*;
// use boring::ssl::SslMethod;
use net::TlsTcpAcceptor;
use std::io::{Error, ErrorKind};

pub trait TlsConfig {
    fn certificate_chain(&self) -> Option<String>;

    fn private_key(&self) -> Option<String>;

    fn certificate(&self) -> Option<String>;

    fn ca_file(&self) -> Option<String>;
}

/// Create an `SslContext` from the given `TlsConfig`. Returns an error if there
/// was any issues during initialization. Otherwise, returns a `SslContext`
/// wrapped in an option, where the `None` variant indicates that TLS should not
/// be used.
pub fn tls_acceptor(config: &dyn TlsConfig) -> Result<Option<TlsTcpAcceptor>, std::io::Error> {
    let mut builder = TlsTcpAcceptor::mozilla_intermediate_v5()?;

    // we use xor here to check if we have an under-specified tls configuration
    if config.private_key().is_some()
        ^ (config.certificate_chain().is_some() || config.certificate().is_some())
    {
        return Err(Error::new(ErrorKind::Other, "incomplete tls configuration"));
    }

    // load the private key
    //
    // NOTE: this is required, so we return `Ok(None)` if it is not specified
    if let Some(f) = config.private_key() {
        builder = builder.private_key_file(f);
    } else {
        return Ok(None);
    }

    // load the ca file
    //
    // NOTE: this is optional, so we do not return `Ok(None)` when it has not
    // been specified
    if let Some(f) = config.ca_file() {
        builder = builder.ca_file(f);
    }

    if let Some(f) = config.certificate() {
        builder = builder.certificate_file(f);
    }

    if let Some(f) = config.certificate_chain() {
        builder = builder.certificate_chain_file(f);
    }

    Ok(Some(builder.build()?))
}
