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

use cc_binding::{option, option_free, option_val_u, OPTION_TYPE_STR};

use super::{Sealed, SingleOption};

/// A string option.
///
/// Note that this type wraps a C string.
#[repr(transparent)]
pub struct Str(option);

unsafe impl Send for Str {}

impl Str {
    /// Get the value of this option as a CStr. If null,
    /// returns none.
    pub fn as_cstr(&self) -> Option<&CStr> {
        let value = self.value();

        if value.is_null() {
            None
        } else {
            Some(unsafe { CStr::from_ptr(value) })
        }
    }

    /// Convert this option to a string if possible. Otherwise
    /// returns None.
    pub fn as_str(&self) -> Option<&str> {
        self.as_cstr().and_then(|s| s.to_str().ok())
    }
}

impl Sealed for Str {}

unsafe impl SingleOption for Str {
    type Value = *mut std::os::raw::c_char;

    fn new(default: Self::Value, name: &'static CStr, desc: &'static CStr) -> Self {
        Self(option {
            name: name.as_ptr() as *mut _,
            set: false,
            type_: OPTION_TYPE_STR,
            default_val: option_val_u { vstr: default },
            val: option_val_u { vstr: default },
            description: desc.as_ptr() as *mut _,
        })
    }
    fn defaulted(name: &'static CStr, desc: &'static CStr) -> Self {
        Self::new(std::ptr::null_mut(), name, desc)
    }

    fn name(&self) -> &'static CStr {
        unsafe { CStr::from_ptr(self.0.name) }
    }
    fn desc(&self) -> &'static CStr {
        unsafe { CStr::from_ptr(self.0.description) }
    }
    fn value(&self) -> Self::Value {
        unsafe { self.0.val.vstr }
    }
    fn default(&self) -> Self::Value {
        unsafe { self.0.default_val.vstr }
    }
    fn is_set(&self) -> bool {
        self.0.set
    }

    fn set_value(&mut self, val: Self::Value) {
        self.0.set = true;
        self.0.val = option_val_u { vstr: val }
    }
}

// TODO(sean): Debug print the string pointer
impl fmt::Debug for Str {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Str")
            .field("name", &self.name())
            .field("desc", &self.desc())
            .field("value", &self.value())
            .field("default", &self.default())
            .field("is_set", &self.is_set())
            .finish()
    }
}

impl Drop for Str {
    fn drop(&mut self) {
        unsafe { option_free(self as *mut _ as *mut option, 1) }
    }
}
