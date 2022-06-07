use crate::{Admin, AdminConfig, Debug, DebugConfig, Klog, KlogConfig};
use core::num::NonZeroU64;
use std::net::AddrParseError;
use std::net::SocketAddr;

use serde::{Deserialize, Serialize};

use std::io::Read;

// struct definitions
#[derive(Clone, Serialize, Deserialize, Debug, Default)]
pub struct MomentoProxyConfig {
    // application modules
    #[serde(default)]
    admin: Admin,
    #[serde(default)]
    proxy: Proxy,
    #[serde(default)]
    cache: Vec<Cache>,
    #[serde(default)]
    debug: Debug,
    #[serde(default)]
    klog: Klog,
}

#[derive(Default, Clone, Copy, Serialize, Deserialize, Debug)]
pub struct Proxy {
    threads: Option<usize>,
}

// definitions
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Cache {
    host: String,
    port: String,
    cache_name: String,
    default_ttl: NonZeroU64,
}

// implementation
impl Cache {
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

    /// Returns the name of the momento cache that requests will be sent to
    pub fn cache_name(&self) -> String {
        self.cache_name.clone()
    }

    /// The default TTL (in seconds) for
    pub fn default_ttl(&self) -> NonZeroU64 {
        self.default_ttl
    }
}

// implementation
impl MomentoProxyConfig {
    pub fn load(file: &str) -> Result<Self, std::io::Error> {
        let mut file = std::fs::File::open(file)?;
        let mut content = String::new();
        file.read_to_string(&mut content)?;
        match toml::from_str(&content) {
            Ok(t) => Ok(t),
            Err(e) => {
                eprintln!("{}", e);
                Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Error parsing config",
                ))
            }
        }
    }

    pub fn caches(&self) -> &[Cache] {
        &self.cache
    }

    pub fn threads(&self) -> Option<usize> {
        self.proxy.threads
    }
}

impl AdminConfig for MomentoProxyConfig {
    fn admin(&self) -> &Admin {
        &self.admin
    }
}

impl DebugConfig for MomentoProxyConfig {
    fn debug(&self) -> &Debug {
        &self.debug
    }
}

impl KlogConfig for MomentoProxyConfig {
    fn klog(&self) -> &Klog {
        &self.klog
    }
}
