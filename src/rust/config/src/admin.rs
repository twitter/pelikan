// Copyright 2020 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use std::net::{AddrParseError, SocketAddr};

use serde::{Deserialize, Serialize};

// constants to define default values
const ADMIN_HOST: &str = "127.0.0.1";
const ADMIN_PORT: &str = "9999";
const ADMIN_HTTP_ENABLED: bool = false;
const ADMIN_HTTP_HOST: &str = "127.0.0.1";
const ADMIN_HTTP_PORT: &str = "9998";
const ADMIN_TIMEOUT: usize = 100;
const ADMIN_NEVENT: usize = 1024;
const ADMIN_TW_TICK: usize = 10;
const ADMIN_TW_CAP: usize = 1000;
const ADMIN_TW_NTICK: usize = 100;
const ADMIN_USE_TLS: bool = false;

// TODO(bmartin): we will eventually migrate to HTTP by default and make the
// legacy admin port as optional. At that time, we should consider consolidating
// the host and port parameters into a single listen address parameter. By using
// Option<> types, we can also eliminate the use of a separate bool to enable
// the legacy admin port, the presence or absence of a listen address being
// enough to determine the desired behavior.

// helper functions for default values
fn host() -> String {
    ADMIN_HOST.to_string()
}

fn port() -> String {
    ADMIN_PORT.to_string()
}

fn http_enabled() -> bool {
    ADMIN_HTTP_ENABLED
}

fn http_host() -> String {
    ADMIN_HTTP_HOST.to_string()
}

fn http_port() -> String {
    ADMIN_HTTP_PORT.to_string()
}

fn timeout() -> usize {
    ADMIN_TIMEOUT
}

fn nevent() -> usize {
    ADMIN_NEVENT
}

fn tw_tick() -> usize {
    ADMIN_TW_TICK
}

fn tw_cap() -> usize {
    ADMIN_TW_CAP
}

fn tw_ntick() -> usize {
    ADMIN_TW_NTICK
}

fn use_tls() -> bool {
    ADMIN_USE_TLS
}

// definitions
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Admin {
    #[serde(default = "host")]
    host: String,
    #[serde(default = "port")]
    port: String,
    #[serde(default = "http_enabled")]
    http_enabled: bool,
    #[serde(default = "http_host")]
    http_host: String,
    #[serde(default = "http_port")]
    http_port: String,
    #[serde(default = "timeout")]
    timeout: usize,
    #[serde(default = "nevent")]
    nevent: usize,
    #[serde(default = "tw_tick")]
    tw_tick: usize,
    #[serde(default = "tw_cap")]
    tw_cap: usize,
    #[serde(default = "tw_ntick")]
    tw_ntick: usize,
    #[serde(default = "use_tls")]
    use_tls: bool,
}

// implementation
impl Admin {
    pub fn host(&self) -> String {
        self.host.clone()
    }

    pub fn port(&self) -> String {
        self.port.clone()
    }

    pub fn http_enabled(&self) -> bool {
        self.http_enabled
    }

    pub fn http_socket_addr(&self) -> Result<SocketAddr, AddrParseError> {
        format!("{}:{}", self.http_host, self.http_port).parse()
    }

    pub fn timeout(&self) -> usize {
        self.timeout
    }

    pub fn nevent(&self) -> usize {
        self.nevent
    }

    pub fn tw_tick(&self) -> usize {
        self.tw_tick
    }

    pub fn tw_cap(&self) -> usize {
        self.tw_cap
    }

    pub fn tw_ntick(&self) -> usize {
        self.tw_ntick
    }

    /// Return the result of parsing the host and port
    pub fn socket_addr(&self) -> Result<SocketAddr, AddrParseError> {
        format!("{}:{}", self.host(), self.port()).parse()
    }

    /// If TLS is configured, the admin port should also use TLS
    pub fn use_tls(&self) -> bool {
        self.use_tls
    }
}

// trait implementations
impl Default for Admin {
    fn default() -> Self {
        Self {
            host: host(),
            port: port(),
            http_enabled: http_enabled(),
            http_host: http_host(),
            http_port: http_port(),
            timeout: timeout(),
            nevent: nevent(),
            tw_tick: tw_tick(),
            tw_cap: tw_cap(),
            tw_ntick: tw_ntick(),
            use_tls: use_tls(),
        }
    }
}

pub trait AdminConfig {
    fn admin(&self) -> &Admin;
}
