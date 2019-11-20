// Copyright (C) 2019 Twitter, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use ccommon::option::*;
use ccommon::Options;

use std::error::Error;
use std::ffi::CStr;
use std::fmt;
use std::net::SocketAddr;
use std::time::Duration;

#[derive(Options)]
#[repr(C)]
pub struct AdminOptions {
    #[option(desc = "admin interface", default = std::ptr::null_mut())]
    pub admin_host: Str,
    #[option(desc = "admin port", default = 9999)]
    pub admin_port: UInt,
    #[option(desc = "debug log flush interval (ms)", default = 500)]
    pub dlog_intvl: UInt,
}

#[derive(Options)]
#[repr(C)]
pub struct ServerOptions {
    #[option(desc = "server interface", default = std::ptr::null_mut())]
    pub server_host: Str,
    #[option(desc = "server port", default = 12321)]
    pub server_port: UInt,
}

impl AdminOptions {
    fn _addr(&self) -> Result<SocketAddr, AddrParseData> {
        let ptr = self.admin_host.value();
        let cstr = if ptr.is_null() {
            None
        } else {
            Some(unsafe { CStr::from_ptr(ptr) })
        };
        let host = cstr.and_then(|s| s.to_str().ok()).unwrap_or("0.0.0.0");
        let port = self.admin_port.value();

        if port > std::u16::MAX as u64 {
            return Err(AddrParseData::InvalidPort(port));
        }

        Ok(SocketAddr::new(host.parse()?, port as u16))
    }

    pub fn addr(&self) -> Result<SocketAddr, AddrParseError> {
        self._addr().map_err(AddrParseError)
    }

    pub fn dlog_intvl(&self) -> Duration {
        Duration::from_millis(self.dlog_intvl.value())
    }
}

impl ServerOptions {
    fn _addr(&self) -> Result<SocketAddr, AddrParseData> {
        let ptr = self.server_host.value();
        let cstr = if ptr.is_null() {
            None
        } else {
            Some(unsafe { CStr::from_ptr(ptr) })
        };
        let host = cstr.and_then(|s| s.to_str().ok()).unwrap_or("0.0.0.0");
        let port = self.server_port.value();

        if port > std::u16::MAX as u64 {
            return Err(AddrParseData::InvalidPort(port));
        }

        Ok(SocketAddr::new(host.parse()?, port as u16))
    }

    pub fn addr(&self) -> Result<SocketAddr, AddrParseError> {
        self._addr().map_err(AddrParseError)
    }
}

pub struct AddrParseError(AddrParseData);

#[derive(Debug)]
enum AddrParseData {
    InvalidIP(std::net::AddrParseError),
    InvalidPort(u64),
}

impl From<std::net::AddrParseError> for AddrParseData {
    fn from(x: std::net::AddrParseError) -> Self {
        Self::InvalidIP(x)
    }
}

impl fmt::Display for AddrParseError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        use self::AddrParseData::*;

        match &self.0 {
            InvalidIP(err) => write!(fmt, "Invalid IP address: {}", err),
            InvalidPort(port) => write!(fmt, "{} is not a valid port number", port),
        }
    }
}

impl fmt::Debug for AddrParseError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_tuple("AddrParseError")
            .field(&format_args!("{}", self))
            .finish()
    }
}

impl Error for AddrParseError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        use self::AddrParseData::*;

        match &self.0 {
            InvalidIP(err) => Some(err),
            _ => None,
        }
    }
}
