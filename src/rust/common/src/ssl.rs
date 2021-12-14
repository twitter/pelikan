// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use boring::x509::X509;
use boring::ssl::*;
use config::TlsConfig;
use std::io::{Error, ErrorKind};

pub fn ssl_context(config: &TlsConfig) -> Result<Option<SslContext>, std::io::Error> {
    let mut builder = SslAcceptor::mozilla_intermediate_v5(SslMethod::tls_server())?;

    // load the private key
    if let Some(f) = config.private_key() {
        builder
            .set_private_key_file(f, SslFiletype::PEM)
            .map_err(|_| Error::new(ErrorKind::Other, "bad private key"))?;
    } else {
        return Ok(None);
    }

    // load the ca file
    if let Some(f) = config.ca_file() {
        builder
            .set_ca_file(f)
            .map_err(|_| Error::new(ErrorKind::Other, "bad ca file"))?;
    } else {
        return Ok(None);
    }

    let chain = config.certificate_chain().is_some();
    let cert = config.certificate().is_some();

    if chain && !cert {
        // assume we have a complete chain: leaf + intermediates + root in one file
        if let Some(f) = config.certificate_chain() {
            builder
                .set_certificate_chain_file(f)
                .map_err(|_| Error::new(ErrorKind::Other, "bad certificate chain"))?;
        } else {
            return Ok(None);
        }
    } else if cert && chain {
        // assume we have the leaf in a standalone file, and the intermediates + root in another file

        // first load the leaf
        if let Some(f) = config.certificate() {
            builder
                .set_certificate_file(f, SslFiletype::PEM)
                .map_err(|_| Error::new(ErrorKind::Other, "bad certificate chain"))?;
        } else {
            return Ok(None);
        }

        // append the rest of the chain
        if let Some(f) = config.certificate_chain() {
            let pem = std::fs::read(f).map_err(|_| Error::new(ErrorKind::Other, "failed to read certificate chain"))?;
            let chain = X509::stack_from_pem(&pem).map_err(|_| Error::new(ErrorKind::Other, "bad certificate chain"))?;
            for cert in chain {
                builder
                    .add_extra_chain_cert(cert)
                    .map_err(|_| Error::new(ErrorKind::Other, "bad certificate in chain"))?;
            }
        } else {
            return Ok(None);
        }

    } else {
        return Err(Error::new(ErrorKind::Other, "no certificate chain provided"));
    }

    Ok(Some(builder.build().into_context()))
}
