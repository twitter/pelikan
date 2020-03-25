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

use std::ffi::CStr;
use std::fmt;
use std::ops::{AddAssign, SubAssign};
use std::sync::atomic::{AtomicU64, Ordering};

use ccommon_sys::{metric, metric_anon_union, METRIC_COUNTER};

use super::private::Sealed;
use super::SingleMetric;

/// An atomic counter metric.
///
/// Exposes `incr` and `decr` operations that can also be
/// used via operators `+=` and `-=`.
#[derive(Copy, Clone)]
#[repr(transparent)]
pub struct Counter(metric);

unsafe impl Send for Counter {}
unsafe impl Sync for Counter {}

impl Counter {
    fn as_ref(&self) -> &AtomicU64 {
        unsafe { &*self.0.data.as_ptr::<AtomicU64>() }
    }

    /// Increment the counter by `n` atomically.
    pub fn incr_n(&self, n: u64) {
        self.as_ref().fetch_add(n, Ordering::Relaxed);
    }

    /// Increment the counter by `1` atomically.
    pub fn incr(&self) {
        self.incr_n(1)
    }

    /// Decrement the counter by `n` atomically.
    pub fn decr_n(&self, n: u64) {
        self.as_ref().fetch_sub(n, Ordering::Relaxed);
    }

    /// Decrement the counter by `1` atomically.
    pub fn decr(&self) {
        self.decr_n(1)
    }

    /// Atomically store a value in the counter.
    pub fn update(&self, val: u64) {
        self.as_ref().store(val, Ordering::Relaxed)
    }

    /// Atomically get the value out of the counter.
    pub fn value(&self) -> u64 {
        self.as_ref().load(Ordering::Relaxed)
    }
}

impl Sealed for Counter {}

unsafe impl SingleMetric for Counter {
    fn new(name: &CStr, desc: &CStr) -> Self {
        Self(metric {
            name: name.as_ptr() as *mut i8,
            desc: desc.as_ptr() as *mut i8,
            type_: METRIC_COUNTER,
            data: metric_anon_union::counter(0),
        })
    }

    fn name(&self) -> &'static CStr {
        unsafe { CStr::from_ptr(self.0.name) }
    }
    fn desc(&self) -> &'static CStr {
        unsafe { CStr::from_ptr(self.0.desc) }
    }
}

impl fmt::Debug for Counter {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Counter")
            .field("name", unsafe { &CStr::from_ptr(self.0.name as *const i8) })
            .field("desc", unsafe { &CStr::from_ptr(self.0.desc as *const i8) })
            .field("counter", &self.value())
            .finish()
    }
}

impl AddAssign<u64> for Counter {
    fn add_assign(&mut self, val: u64) {
        self.incr_n(val);
    }
}

impl SubAssign<u64> for Counter {
    fn sub_assign(&mut self, val: u64) {
        self.decr_n(val);
    }
}
