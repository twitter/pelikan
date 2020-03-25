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
use std::sync::atomic::{AtomicI64, Ordering};

use cc_binding::{metric, metric_anon_union, METRIC_GAUGE};

use super::private::Sealed;
use super::SingleMetric;

/// A gauge metric that can be updated atomically.
///
/// Exposes `incr`, `decr`, and `update`. `incr` and `decr`
/// can be used through operators `+=` and `-=`.
#[derive(Copy, Clone)]
#[repr(transparent)]
pub struct Gauge(metric);

impl Gauge {
    fn as_ref(&self) -> &AtomicI64 {
        unsafe { &*self.0.data.as_ptr::<AtomicI64>() }
    }

    /// Increment the gauge atomically by `n`.
    pub fn incr_n(&self, n: i64) {
        self.as_ref().fetch_add(n, Ordering::Relaxed);
    }

    /// Increment the gauge atomically by `1`.
    pub fn incr(&self) {
        self.incr_n(1)
    }

    /// Decrement the gauge atomically by `n`.
    pub fn decr_n(&self, n: i64) {
        self.as_ref().fetch_sub(n, Ordering::Relaxed);
    }

    /// Decrement the gauge atomically by `1`.
    pub fn decr(&self) {
        self.decr_n(1)
    }

    /// Set the value of the gauge atomically.
    pub fn update(&self, val: i64) {
        self.as_ref().store(val, Ordering::Relaxed)
    }

    /// Get the value of the gauge atomically.
    pub fn value(&self) -> i64 {
        self.as_ref().load(Ordering::Relaxed)
    }
}

impl Sealed for Gauge {}

unsafe impl Send for Gauge {}
unsafe impl Sync for Gauge {}

unsafe impl SingleMetric for Gauge {
    fn new(name: &CStr, desc: &CStr) -> Self {
        Self(metric {
            name: name.as_ptr() as *mut i8,
            desc: desc.as_ptr() as *mut i8,
            type_: METRIC_GAUGE,
            data: metric_anon_union::gauge(0),
        })
    }

    fn name(&self) -> &'static CStr {
        unsafe { CStr::from_ptr(self.0.name) }
    }
    fn desc(&self) -> &'static CStr {
        unsafe { CStr::from_ptr(self.0.desc) }
    }
}

impl fmt::Debug for Gauge {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Counter")
            .field("name", unsafe { &CStr::from_ptr(self.0.name as *const i8) })
            .field("desc", unsafe { &CStr::from_ptr(self.0.desc as *const i8) })
            .field("counter", &self.value())
            .finish()
    }
}

impl AddAssign<i64> for Gauge {
    fn add_assign(&mut self, val: i64) {
        self.incr_n(val);
    }
}

impl SubAssign<i64> for Gauge {
    fn sub_assign(&mut self, val: i64) {
        self.decr_n(val);
    }
}
