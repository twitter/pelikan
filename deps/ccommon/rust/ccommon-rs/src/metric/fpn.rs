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
use std::sync::atomic::{AtomicU64, Ordering};

use cc_binding::{metric, metric_anon_union, METRIC_FPN};

use super::private::Sealed;
use super::SingleMetric;

/// A `f64` metric that can be updated atomically.
#[derive(Copy, Clone)]
#[repr(transparent)]
pub struct Fpn(metric);

impl Fpn {
    /// Get the value atomically.
    pub fn value(&self) -> f64 {
        f64::from_bits(unsafe { (*self.0.data.as_ptr::<AtomicU64>()).load(Ordering::Relaxed) })
    }

    /// Update the value atomically.
    pub fn update(&self, val: f64) {
        unsafe { (*self.0.data.as_ptr::<AtomicU64>()).store(val.to_bits(), Ordering::Relaxed) }
    }
}

impl Sealed for Fpn {}

unsafe impl Send for Fpn {}
unsafe impl Sync for Fpn {}

unsafe impl SingleMetric for Fpn {
    fn new(name: &CStr, desc: &CStr) -> Self {
        Self(metric {
            name: name.as_ptr() as *mut i8,
            desc: desc.as_ptr() as *mut i8,
            type_: METRIC_FPN,
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

impl fmt::Debug for Fpn {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Fpn")
            .field("name", unsafe { &CStr::from_ptr(self.0.name as *const i8) })
            .field("desc", unsafe { &CStr::from_ptr(self.0.desc as *const i8) })
            .field("fpn", &self.value())
            .finish()
    }
}
