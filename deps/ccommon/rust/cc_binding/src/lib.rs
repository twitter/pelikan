// ccommon - a cache common library.
// Copyright (C) 2018 Twitter, Inc.
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

//! This module contains the bindgen created classes.
//! PRO-TIP: If you want to look at the generated code, you can find it with:
//!
//! ```ignore
//! $ find . -name bindgen.rs
//! ```
//!

#![allow(unknown_lints)]
#![allow(clippy)]
#![allow(clippy_pedantic)]
#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]

#[path = "metric.rs"]
mod metric_impl;

pub use self::metric_impl::{metric, metric_anon_union};

use libc::{addrinfo, timespec, FILE};

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

pub const DEBUG_LOG_FILE: *mut i8 = std::ptr::null_mut();

/// default log file
pub const STATS_LOG_FILE: *mut i8 = std::ptr::null_mut();
/// default log buf size
pub const STATS_LOG_NBUF: u32 = 0;

// Option methods
pub unsafe fn option_bool(opt: *mut option) -> bool {
    return (*opt).val.vbool;
}

pub unsafe fn option_uint(opt: *mut option) -> u64 {
    return (*opt).val.vuint;
}

pub unsafe fn option_fpn(opt: *mut option) -> f64 {
    return (*opt).val.vfpn;
}

pub unsafe fn option_str(opt: *mut option) -> *mut i8 {
    return (*opt).val.vstr;
}
