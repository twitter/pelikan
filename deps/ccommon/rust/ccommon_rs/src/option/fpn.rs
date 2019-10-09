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

use cc_binding::{option, option_val_u, OPTION_TYPE_FPN};

use super::{Sealed, SingleOption};

/// A floating-point option.
#[derive(Copy, Clone)]
#[repr(transparent)]
pub struct Float(option);

impl Sealed for Float {}

unsafe impl SingleOption for Float {
    type Value = f64;

    fn new(default: Self::Value, name: &'static CStr, desc: &'static CStr) -> Self {
        Self(option {
            name: name.as_ptr() as *mut _,
            set: false,
            type_: OPTION_TYPE_FPN,
            default_val: option_val_u { vfpn: default },
            val: option_val_u { vfpn: default },
            description: desc.as_ptr() as *mut _,
        })
    }
    fn defaulted(name: &'static CStr, desc: &'static CStr) -> Self {
        Self::new(Default::default(), name, desc)
    }

    fn name(&self) -> &'static CStr {
        unsafe { CStr::from_ptr(self.0.name) }
    }
    fn desc(&self) -> &'static CStr {
        unsafe { CStr::from_ptr(self.0.description) }
    }
    fn value(&self) -> Self::Value {
        unsafe { self.0.val.vfpn }
    }
    fn default(&self) -> Self::Value {
        unsafe { self.0.default_val.vfpn }
    }
    fn is_set(&self) -> bool {
        self.0.set
    }

    fn set_value(&mut self, val: Self::Value) {
        self.0.set = true;
        self.0.val = option_val_u { vfpn: val }
    }
}

impl fmt::Debug for Float {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Float")
            .field("name", &self.name())
            .field("desc", &self.desc())
            .field("value", &self.value())
            .field("default", &self.default())
            .field("is_set", &self.is_set())
            .finish()
    }
}
