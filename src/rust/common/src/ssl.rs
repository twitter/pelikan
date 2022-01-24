// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

pub use boring::ssl::*;
use boring::x509::X509;
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
pub fn ssl_context(config: &dyn TlsConfig) -> Result<Option<SslContext>, std::io::Error> {
    let mut builder = SslAcceptor::mozilla_intermediate_v5(SslMethod::tls_server())?;

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
        builder
            .set_private_key_file(f, SslFiletype::PEM)
            .map_err(|_| Error::new(ErrorKind::Other, "bad private key"))?;
    } else {
        return Ok(None);
    }

    // load the ca file
    //
    // NOTE: this is optional, so we do not return `Ok(None)` when it has not
    // been specified
    if let Some(f) = config.ca_file() {
        builder
            .set_ca_file(f)
            .map_err(|_| Error::new(ErrorKind::Other, "bad ca file"))?;
    }

    match (config.certificate_chain(), config.certificate()) {
        (Some(chain), Some(cert)) => {
            // assume we have the leaf in a standalone file, and the
            // intermediates + root in another file

            // first load the leaf
            builder
                .set_certificate_file(cert, SslFiletype::PEM)
                .map_err(|_| Error::new(ErrorKind::Other, "bad certificate file"))?;

            // append the rest of the chain
            let pem = std::fs::read(chain)
                .map_err(|_| Error::new(ErrorKind::Other, "failed to read certificate chain"))?;
            let chain = X509::stack_from_pem(&pem)
                .map_err(|_| Error::new(ErrorKind::Other, "bad certificate chain"))?;
            for cert in chain {
                builder
                    .add_extra_chain_cert(cert)
                    .map_err(|_| Error::new(ErrorKind::Other, "bad certificate in chain"))?;
            }
        }
        (Some(chain), None) => {
            // assume we have a complete chain: leaf + intermediates + root in
            // one file

            // load the entire chain
            builder
                .set_certificate_chain_file(chain)
                .map_err(|_| Error::new(ErrorKind::Other, "bad certificate chain"))?;
        }
        (None, Some(cert)) => {
            // this will just load the leaf certificate from the file
            builder
                .set_certificate_file(cert, SslFiletype::PEM)
                .map_err(|_| Error::new(ErrorKind::Other, "bad certificate file"))?;
        }
        (None, None) => {
            // if we have neither a chain nor a leaf cert to load, we return no
            // ssl context
            return Ok(None);
        }
    }

    Ok(Some(builder.build().into_context()))
}
