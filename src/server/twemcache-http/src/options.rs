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

use ccommon::{option::*, Options};
use ccommon_sys::{
    array_options_st, buf_options_st, dbuf_options_st, debug_options_st, sockio_options_st,
    stats_log_options_st,
};
use pelikan_sys::{
    hotkey::hotkey_options_st, protocol::memcache::*, storage::slab::slab_options_st,
    time::time_options_st,
};
use rustcore::errors::AddrParseError;
use rustcore::{AdminOptions, ListenerOptions};

use crate::memcached::sys::process_options_st;

use std::ffi::CStr;
use std::net::SocketAddr;

#[rustfmt::skip]
#[repr(C)]
#[derive(Options)]
pub struct Options {
    // top-level
    pub http:       HttpOptions,
    pub process:    process_options_st,
    pub twemcache:  TwemcacheOptions,
    
    // Application Modules
    pub admin:      AdminOptions,
    pub listener:   ListenerOptions,
    pub time:       time_options_st,
    pub klog:       klog_options_st,
    pub request:    request_options_st,
    pub response:   response_options_st,
    pub slab:       slab_options_st,
    pub hotkey:     hotkey_options_st,
    pub server:     ServerOptions,
    
    // ccommon libraries
    pub array:      array_options_st,
    pub buf:        buf_options_st,
    pub dbuf:       dbuf_options_st,
    pub debug:      debug_options_st,
    pub sockio:     sockio_options_st,
    pub stats_log:  stats_log_options_st,
}

#[repr(C)]
#[derive(Options)]
pub struct ServerOptions {
    #[option(desc = "daemonize the process", default = false)]
    pub daemonize: Bool,
    #[option(desc="file storing the pid", default = std::ptr::null_mut())]
    pub pid_filename: Str,
}

#[repr(C)]
#[derive(Options)]
pub struct HttpOptions {
    #[option(desc = "http interface", default = std::ptr::null_mut())]
    pub http_host: Str,
    #[option(desc = "http port", default = 4779)]
    pub http_port: UInt,
}

impl HttpOptions {
    pub fn addr(&self) -> std::result::Result<SocketAddr, AddrParseError> {
        let ptr = self.http_host.value();
        let cstr = if ptr.is_null() {
            None
        } else {
            Some(unsafe { CStr::from_ptr(ptr) })
        };
        let host = cstr.and_then(|s| s.to_str().ok()).unwrap_or("0.0.0.0");
        let port = self.http_port.value();

        if port > std::u16::MAX as u64 {
            return Err(AddrParseError::InvalidPort(port));
        }

        Ok(SocketAddr::new(host.parse()?, port as u16))
    }
}

#[repr(C)]
#[derive(Options)]
pub struct TwemcacheOptions {
    #[option(desc = "twemcache interface", default = std::ptr::null_mut())]
    pub twemcache_host: Str,
    #[option(desc = "twemcache port", default = 12321)]
    pub twemcache_port: UInt,
}

impl TwemcacheOptions {
    pub fn addr(&self) -> std::result::Result<SocketAddr, AddrParseError> {
        let ptr = self.twemcache_host.value();
        let cstr = if ptr.is_null() {
            None
        } else {
            Some(unsafe { CStr::from_ptr(ptr) })
        };
        let host = cstr.and_then(|s| s.to_str().ok()).unwrap_or("0.0.0.0");
        let port = self.twemcache_port.value();

        if port > std::u16::MAX as u64 {
            return Err(AddrParseError::InvalidPort(port));
        }

        Ok(SocketAddr::new(host.parse()?, port as u16))
    }
}

#[test]
fn test_settings_size_is_multiple_of_option_size() {
    use ccommon_sys::option;
    use std::mem;

    let option_size = mem::size_of::<option>();
    assert_eq!(mem::size_of::<Options>() % option_size, 0);
}
