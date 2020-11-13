// Copyright 2020 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use serde::{Deserialize, Serialize};

use std::net::{AddrParseError, SocketAddr};

// constants to define default values
const SERVER_HOST: &str = "0.0.0.0";
const SERVER_PORT: &str = "12321";
const SERVER_TIMEOUT: usize = 100;
const SERVER_NEVENT: usize = 1024;

// helper functions
fn host() -> String {
    SERVER_HOST.to_string()
}

fn port() -> String {
    SERVER_PORT.to_string()
}

fn timeout() -> usize {
    SERVER_TIMEOUT
}

fn nevent() -> usize {
    SERVER_NEVENT
}

// definitions
#[derive(Serialize, Deserialize, Debug)]
pub struct ServerConfig {
    #[serde(default = "host")]
    host: String,
    #[serde(default = "port")]
    port: String,
    #[serde(default = "timeout")]
    timeout: usize,
    #[serde(default = "nevent")]
    nevent: usize,
}

// implementation
impl ServerConfig {
    /// Host address to listen on
    pub fn host(&self) -> String {
        self.host.clone()
    }

    /// Port to listen on
    pub fn port(&self) -> String {
        self.port.clone()
    }

    /// Return the result of parsing the host and port
    pub fn socket_addr(&self) -> Result<SocketAddr, AddrParseError> {
        format!("{}:{}", self.host(), self.port()).parse()
    }

    /// The poll timeout in milliseconds
    pub fn timeout(&self) -> usize {
        self.timeout
    }

    /// Maximum events to accept in one poll
    pub fn nevent(&self) -> usize {
        self.nevent
    }
}

// trait implementations
impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: host(),
            port: port(),
            timeout: timeout(),
            nevent: nevent(),
        }
    }
}
