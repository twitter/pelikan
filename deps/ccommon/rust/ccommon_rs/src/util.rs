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


use std::ffi::CStr;
use std::fs;
use std::os::raw::c_char;

use cc_binding::{metric, option};

/// Recursively removes files and directories under `path` before removing `path` itself.
/// Returns 0 on success and -1 on error. `errno` will be set to the cause of the failure.
#[no_mangle]
pub unsafe extern "C" fn cc_util_rm_rf_rs(path: *const c_char) -> i32 {
    assert!(!path.is_null());

    let s =
        match CStr::from_ptr(path as *mut c_char).to_str() {
            Ok(s) => s,
            Err(err) => {
                eprintln!("ERROR: cc_util_rm_rf_rs: {:#?}", err);
                return -1
            }
        };

    match fs::remove_dir_all(s) {
        Ok(()) => 0,
        Err(err) => {
            eprintln!("ERROR, cc_util_rm_rf_rs {:#?}", err);
            -1
        }
    }
}

pub unsafe trait AsMetricArray {
    unsafe fn as_metric_array<'a>(&'a self) -> &'a [metric] {
        use std::mem::{size_of, size_of_val};
        use std::slice;

        // Panic in cases that obviously won't work
        assert_eq!(size_of_val(self) % size_of::<metric>(), 0);

        let count = size_of_val(self) / size_of::<metric>();
        slice::from_raw_parts(
            self as *const _ as *const metric,
            count
        )
    }
    unsafe fn as_metric_array_mut<'a>(&'a mut self) -> &'a mut [metric] {
        use std::mem::{size_of, size_of_val};
        use std::slice;

        // Panic in cases that obviously won't work
        assert_eq!(size_of_val(self) % size_of::<metric>(), 0);

        let count = size_of_val(self) / size_of::<metric>();
        slice::from_raw_parts_mut(
            self as *mut _ as *mut metric,
            count
        )
    }

    unsafe fn describe_all(&self) {
        let slice = self.as_metric_array();

        use cc_binding::metric_describe_all;
        metric_describe_all(
            slice.as_ptr() as *mut _,
            slice.len() as u32
        )
    }
}

pub unsafe trait AsOptionArray {
    unsafe fn as_option_array<'a>(&'a self) -> &'a [option] {
        use std::mem::{size_of, size_of_val};
        use std::slice;

        // Panic in cases that obviously won't work
        assert_eq!(size_of_val(self) % size_of::<option>(), 0);

        let count = size_of_val(self) / size_of::<option>();
        slice::from_raw_parts(
            self as *const _ as *const option,
            count
        )
    }
    unsafe fn as_option_array_mut<'a>(&'a mut self) -> &'a mut [option] {
        use std::mem::{size_of, size_of_val};
        use std::slice;

        // Panic in cases that obviously won't work
        assert_eq!(size_of_val(self) % size_of::<option>(), 0);

        let count = size_of_val(self) / size_of::<option>();
        slice::from_raw_parts_mut(
            self as *mut _ as *mut option,
            count
        )
    }

    unsafe fn describe_all(&self) {
        let slice = self.as_option_array();

        use cc_binding::option_describe_all;
        option_describe_all(
            slice.as_ptr() as *mut _,
            slice.len() as u32
        )
    }
}

