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

use ccommon_sys::option;
use libc::time_t;

include!(concat!(env!("OUT_DIR"), "/time.rs"));

pub const TIME_MEMCACHE_MAXDELTA_SEC: time_t = 60 * 60 * 32 * 24;
pub const TIME_MEMCACHE_MAXDELTA_MS: time_t = TIME_MEMCACHE_MAXDELTA_SEC * 1000;
pub const TIME_MEMCACHE_MAXDELTA_US: time_t = TIME_MEMCACHE_MAXDELTA_MS * 1000;
pub const TIME_MEMCACHE_MAXDELTA_NS: time_t = TIME_MEMCACHE_MAXDELTA_US * 1000;

unsafe fn atomic_load_i64(val: &i64) -> i64 {
    use std::sync::atomic::{AtomicI64, Ordering};

    let atomic = &*(val as *const i64 as *const AtomicI64);
    atomic.load(Ordering::Relaxed)
}
unsafe fn atomic_load_i32(val: &i32) -> i32 {
    use std::sync::atomic::{AtomicI32, Ordering};

    let atomic = &*(val as *const i32 as *const AtomicI32);
    atomic.load(Ordering::Relaxed)
}

pub unsafe fn time_started() -> time_t {
    atomic_load_i64(&time_start)
}

pub unsafe fn time_proc_sec() -> proc_time_i {
    atomic_load_i32(&proc_sec)
}
pub unsafe fn time_proc_ms() -> proc_time_fine_i {
    atomic_load_i64(&proc_ms)
}
pub unsafe fn time_proc_us() -> proc_time_fine_i {
    atomic_load_i64(&proc_us)
}
pub unsafe fn time_proc_ns() -> proc_time_fine_i {
    atomic_load_i64(&proc_ns)
}

pub unsafe fn time_unix2proc_sec(t: unix_time_u) -> proc_time_i {
    t as proc_time_i - time_started() as proc_time_i
}

pub unsafe fn time_delta2proc_sec(t: delta_time_i) -> proc_time_i {
    t as proc_time_i + time_proc_sec()
}

pub unsafe fn time_memcache2proc_sec(t: memcache_time_u) -> proc_time_i {
    if t == 0 {
        return std::i32::MAX;
    }

    if t as time_t > TIME_MEMCACHE_MAXDELTA_SEC {
        time_unix2proc_sec(t as unix_time_u)
    } else {
        time_delta2proc_sec(t as delta_time_i)
    }
}

pub unsafe fn time_convert_proc_sec(t: time_i) -> proc_time_i {
    match time_type as u32 {
        TIME_UNIX => time_unix2proc_sec(t as unix_time_u),
        TIME_DELTA => time_delta2proc_sec(t as delta_time_i),
        TIME_MEMCACHE => time_memcache2proc_sec(t as memcache_time_u),
        _ => unreachable!(),
    }
}

unsafe impl ccommon::option::Options for time_options_st {
    fn new() -> Self {
        init_option! {
            Self;
            ACTION(time_type, OPTION_TYPE_UINT, TIME_UNIX as u64, "Expiry timestamp mode")
        }
    }
}
