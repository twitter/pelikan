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

use ccommon_sys::{
    _cc_alloc, option, OPTION_TYPE_BOOL, OPTION_TYPE_FPN, OPTION_TYPE_STR, OPTION_TYPE_UINT,
};

use std::convert::TryInto;
use std::ffi::CStr;
use std::fmt;

macro_rules! c_str {
    ($s:expr) => {
        concat!($s, "\0").as_ptr() as *const i8
    };
}

#[derive(Debug)]
pub struct OutOfMemoryError(());

impl fmt::Display for OutOfMemoryError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "Out of memory")
    }
}

impl std::error::Error for OutOfMemoryError {}

/// Initialize an option to it's default value.
///
/// # Safety
/// This function assumes that the description and name
/// pointers within the options always point to a valid
/// C string. It also assumes that the value for string
/// type options is either a valid C string or null.
pub unsafe fn option_default(opt: &mut option) -> Result<(), OutOfMemoryError> {
    use std::mem::MaybeUninit;

    opt.set = true;

    match opt.type_ {
        OPTION_TYPE_BOOL => opt.val.vbool = opt.default_val.vbool,
        OPTION_TYPE_UINT => opt.val.vuint = opt.default_val.vuint,
        OPTION_TYPE_FPN => opt.val.vfpn = opt.default_val.vfpn,
        OPTION_TYPE_STR => {
            let default = opt.default_val.vstr;

            if default.is_null() {
                opt.val.vstr = default;
            } else {
                let s = CStr::from_ptr(default);
                let bytes = s.to_bytes_with_nul();
                let mem = _cc_alloc(
                    bytes.len().try_into().unwrap(),
                    c_str!(module_path!()),
                    line!() as std::os::raw::c_int,
                ) as *mut MaybeUninit<u8>;

                if mem.is_null() {
                    return Err(OutOfMemoryError(()));
                }

                std::ptr::copy_nonoverlapping(bytes.as_ptr(), mem as *mut u8, bytes.len());

                opt.val.vstr = mem as *mut libc::c_char
            }
        }
        _ => opt.val = opt.default_val,
    };

    Ok(())
}

/// Initialize all the options to their default values.
///
/// # Safety
/// This function assumes that the description and name
/// pointers within the options always point to a valid
/// C string. It also assumes that the value for string
/// type options is either a valid C string or null.
pub unsafe fn option_load_default(options: &mut [option]) -> Result<(), OutOfMemoryError> {
    for opt in options.iter_mut() {
        option_default(opt)?;
    }

    Ok(())
}
