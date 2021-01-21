// Copyright 2020 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use boring::ssl::{SslAcceptor, SslContext, SslFiletype, SslMethod};
use config::PingserverConfig;

use std::sync::Arc;

pub enum Message {
    Shutdown,
}

pub fn ssl_context(config: &Arc<PingserverConfig>) -> Result<Option<SslContext>, std::io::Error> {
    let mut builder = SslAcceptor::mozilla_intermediate_v5(SslMethod::tls_server())?;

    if let Some(f) = config.tls().certificate_chain() {
        builder.set_ca_file(f);
    } else {
        return Ok(None);
    }

    if let Some(f) = config.tls().certificate() {
        builder.set_certificate_file(f, SslFiletype::PEM);
    } else {
        return Ok(None);
    }

    if let Some(f) = config.tls().private_key() {
        builder.set_private_key_file(f, SslFiletype::PEM);
    } else {
        return Ok(None);
    }

    Ok(Some(builder.build().into_context()))
}
