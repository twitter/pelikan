use boring::ssl::*;
use config::TlsConfig;
use std::io::{Error, ErrorKind};

pub fn ssl_context(config: &TlsConfig) -> Result<Option<SslContext>, std::io::Error> {
    let mut builder = SslAcceptor::mozilla_intermediate_v5(SslMethod::tls_server())?;

    if let Some(f) = config.certificate_chain() {
        builder
            .set_ca_file(f)
            .map_err(|_| Error::new(ErrorKind::Other, "bad certificate chain"))?;
    } else {
        return Ok(None);
    }

    if let Some(f) = config.certificate() {
        builder
            .set_certificate_file(f, SslFiletype::PEM)
            .map_err(|_| Error::new(ErrorKind::Other, "bad certificate"))?;
    } else {
        return Ok(None);
    }

    if let Some(f) = config.private_key() {
        builder
            .set_private_key_file(f, SslFiletype::PEM)
            .map_err(|_| Error::new(ErrorKind::Other, "bad private key"))?;
    } else {
        return Ok(None);
    }

    Ok(Some(builder.build().into_context()))
}
