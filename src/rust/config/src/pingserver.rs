// Copyright 2020 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::*;

use serde::{Deserialize, Serialize};

use std::io::Read;

// constants to define default values
const DAEMONIZE: bool = false;
const PID_FILENAME: Option<String> = None;
const DLOG_INTERVAL: usize = 500;

// helper functions
fn daemonize() -> bool {
    DAEMONIZE
}

fn pid_filename() -> Option<String> {
    PID_FILENAME
}

fn dlog_interval() -> usize {
    DLOG_INTERVAL
}

// struct definitions
#[derive(Serialize, Deserialize, Debug)]
pub struct PingserverConfig {
    // top-level
    #[serde(default = "daemonize")]
    daemonize: bool,
    #[serde(default = "pid_filename")]
    pid_filename: Option<String>,
    #[serde(default = "dlog_interval")]
    dlog_interval: usize,

    // application modules
    #[serde(default)]
    admin: AdminConfig,
    #[serde(default)]
    server: ServerConfig,
    #[serde(default)]
    worker: WorkerConfig,
    #[serde(default)]
    time: TimeConfig,
    #[serde(default)]
    tls: TlsConfig,

    // ccommon
    #[serde(default)]
    buf: BufConfig,
    #[serde(default)]
    debug: DebugConfig,
    #[serde(default)]
    sockio: SockioConfig,
    #[serde(default)]
    tcp: TcpConfig,
}

// implementation
impl PingserverConfig {
    pub fn load(file: &str) -> Result<PingserverConfig, std::io::Error> {
        let mut file = std::fs::File::open(file)?;
        let mut content = String::new();
        file.read_to_string(&mut content)?;
        match toml::from_str(&content) {
            Ok(t) => Ok(t),
            Err(e) => {
                error!("{}", e);
                Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Error parsing config",
                ))
            }
        }
    }

    pub fn daemonize(&self) -> bool {
        self.daemonize
    }

    pub fn pid_filename(&self) -> Option<String> {
        self.pid_filename.clone()
    }

    pub fn dlog_interval(&self) -> usize {
        self.dlog_interval
    }

    pub fn admin(&self) -> &AdminConfig {
        &self.admin
    }

    pub fn server(&self) -> &ServerConfig {
        &self.server
    }

    pub fn worker(&self) -> &WorkerConfig {
        &self.worker
    }

    pub fn time(&self) -> &TimeConfig {
        &self.time
    }

    pub fn buf(&self) -> &BufConfig {
        &self.buf
    }

    pub fn debug(&self) -> &DebugConfig {
        &self.debug
    }

    pub fn sockio(&self) -> &SockioConfig {
        &self.sockio
    }

    pub fn tcp(&self) -> &TcpConfig {
        &self.tcp
    }

    pub fn tls(&self) -> &TlsConfig {
        &self.tls
    }
}

// trait implementations
impl Default for PingserverConfig {
    fn default() -> Self {
        Self {
            daemonize: daemonize(),
            pid_filename: pid_filename(),
            dlog_interval: dlog_interval(),

            admin: Default::default(),
            server: Default::default(),
            worker: Default::default(),
            time: Default::default(),

            buf: Default::default(),
            debug: Default::default(),
            sockio: Default::default(),
            tcp: Default::default(),
            tls: Default::default(),
        }
    }
}
