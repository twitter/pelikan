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

//! This module contains the generated bindings for the `ccommon` library.
//!
//! PRO-TIP: To view the generated code, look in `src/bindings.rs` after running
//! the build.

#![allow(
    unknown_lints,
    clippy::all,
    non_upper_case_globals,
    non_camel_case_types,
    non_snake_case,
    dead_code
)]

#[path = "metric.rs"]
mod metric_impl;

pub use metric_impl::{metric, metric_anon_union};

use libc::{addrinfo, timespec, FILE};

include!("bindings.rs");

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
