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

mod tcp;

pub use self::tcp::{tcp_listener, TcpListenerMetrics};

use std::ffi::CStr;
use std::net::SocketAddr;

use ccommon::{option::*, Options};

use crate::errors::AddrParseError;

#[derive(Options)]
#[repr(C)]
pub struct ListenerOptions {
    #[option(desc = "server interface", default = std::ptr::null_mut())]
    pub server_host: Str,
    #[option(desc = "server port", default = 12321)]
    pub server_port: UInt,
}

impl ListenerOptions {
    pub fn addr(&self) -> Result<SocketAddr, AddrParseError> {
        let ptr = self.server_host.value();
        let cstr = if ptr.is_null() {
            None
        } else {
            Some(unsafe { CStr::from_ptr(ptr) })
        };
        let host = cstr.and_then(|s| s.to_str().ok()).unwrap_or("0.0.0.0");
        let port = self.server_port.value();

        if port > std::u16::MAX as u64 {
            return Err(AddrParseError::InvalidPort(port));
        }

        Ok(SocketAddr::new(host.parse()?, port as u16))
    }
}
