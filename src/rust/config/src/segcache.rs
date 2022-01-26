// Copyright 2021 Twitter, Inc.
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
pub struct SegcacheConfig {
    // top-level
    #[serde(default = "daemonize")]
    daemonize: bool,
    #[serde(default = "pid_filename")]
    pid_filename: Option<String>,
    #[serde(default = "dlog_interval")]
    dlog_interval: usize,

    // application modules
    #[serde(default)]
    admin: Admin,
    #[serde(default)]
    server: Server,
    #[serde(default)]
    worker: Worker,
    #[serde(default)]
    time: Time,
    #[serde(default)]
    tls: Tls,
    #[serde(default)]
    seg: Seg,

    // ccommon
    #[serde(default)]
    buf: Buf,
    #[serde(default)]
    debug: Debug,
    #[serde(default)]
    klog: Klog,
    #[serde(default)]
    sockio: Sockio,
    #[serde(default)]
    tcp: Tcp,
}

// implementation
impl SegcacheConfig {
    pub fn load(file: &str) -> Result<Self, std::io::Error> {
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
}

impl AdminConfig for SegcacheConfig {
    fn admin(&self) -> &Admin {
        &self.admin
    }
}

impl BufConfig for SegcacheConfig {
    fn buf(&self) -> &Buf {
        &self.buf
    }
}

impl DebugConfig for SegcacheConfig {
    fn debug(&self) -> &Debug {
        &self.debug
    }
}

impl KlogConfig for SegcacheConfig {
    fn klog(&self) -> &Klog {
        &self.klog
    }
}

impl SegConfig for SegcacheConfig {
    fn seg(&self) -> &Seg {
        &self.seg
    }
}

impl ServerConfig for SegcacheConfig {
    fn server(&self) -> &Server {
        &self.server
    }
}

impl SockioConfig for SegcacheConfig {
    fn sockio(&self) -> &Sockio {
        &self.sockio
    }
}

impl TcpConfig for SegcacheConfig {
    fn tcp(&self) -> &Tcp {
        &self.tcp
    }
}

impl TimeConfig for SegcacheConfig {
    fn time(&self) -> &Time {
        &self.time
    }
}

impl TlsConfig for SegcacheConfig {
    fn tls(&self) -> &Tls {
        &self.tls
    }
}

impl WorkerConfig for SegcacheConfig {
    fn worker(&self) -> &Worker {
        &self.worker
    }

    fn worker_mut(&mut self) -> &mut Worker {
        &mut self.worker
    }
}

// trait implementations
impl Default for SegcacheConfig {
    fn default() -> Self {
        Self {
            daemonize: daemonize(),
            pid_filename: pid_filename(),
            dlog_interval: dlog_interval(),

            admin: Default::default(),
            server: Default::default(),
            worker: Default::default(),
            time: Default::default(),
            seg: Default::default(),

            buf: Default::default(),
            debug: Default::default(),
            klog: Default::default(),
            sockio: Default::default(),
            tcp: Default::default(),
            tls: Default::default(),
        }
    }
}
