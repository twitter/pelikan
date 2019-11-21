// ccommon - a cache common library.
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
use ccommon_sys::{
    array_options_st, buf_options_st, dbuf_options_st, debug_options_st, sockio_options_st,
    tcp_options_st,
};
use pelikan_sys::{core::worker_options_st, time::time_options_st};

use rustcore::{AdminOptions, ListenerOptions};

#[repr(C)]
#[derive(Options)]
pub struct PingServerOptions {
    #[option(desc = "daemonize the process", default = false)]
    pub daemonize: Bool,
    #[option(desc="file storing the pid", default = std::ptr::null_mut())]
    pub pid_filename: Str,
}

#[rustfmt::skip]
#[repr(C)]
#[derive(Options)]
pub struct Settings {
    // top-level
    pub pingserver: PingServerOptions,
    // application modules
    pub admin:      AdminOptions,
    pub listener:   ListenerOptions,
    pub worker:     worker_options_st,
    pub time:       time_options_st,
    // ccommon libraries
    pub array:      array_options_st,
    pub buf:        buf_options_st,
    pub dbuf:       dbuf_options_st,
    pub debug:      debug_options_st,
    pub sockio:     sockio_options_st,
    pub tcp:        tcp_options_st
}

#[test]
fn test_settings_size_is_multiple_of_option_size() {
    use ccommon_sys::option;
    use std::mem;

    let option_size = mem::size_of::<option>();
    assert_eq!(mem::size_of::<Settings>() % option_size, 0);
}
