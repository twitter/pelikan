// Copyright 2020 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use std::net::{AddrParseError, SocketAddr};

use serde::{Deserialize, Serialize};

// constants to define default values
const ADMIN_HOST: &str = "0.0.0.0";
const ADMIN_PORT: &str = "9999";
const ADMIN_TIMEOUT: usize = 100;
const ADMIN_NEVENT: usize = 1024;
const ADMIN_TW_TICK: usize = 10;
const ADMIN_TW_CAP: usize = 1000;
const ADMIN_TW_NTICK: usize = 100;

// helper functions for default values
fn host() -> String {
    ADMIN_HOST.to_string()
}

fn port() -> String {
    ADMIN_PORT.to_string()
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

// definitions
#[derive(Serialize, Deserialize, Debug)]
pub struct AdminConfig {
    #[serde(default = "host")]
    host: String,
    #[serde(default = "port")]
    port: String,
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
}

// implementation
impl AdminConfig {
    pub fn host(&self) -> String {
        self.host.clone()
    }

    pub fn port(&self) -> String {
        self.port.clone()
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
}

// trait implementations
impl Default for AdminConfig {
    fn default() -> Self {
        Self {
            host: host(),
            port: port(),
            timeout: timeout(),
            nevent: nevent(),
            tw_tick: tw_tick(),
            tw_cap: tw_cap(),
            tw_ntick: tw_ntick(),
        }
    }
}
