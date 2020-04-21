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

use ccommon_sys::{option, rstatus_i, CC_ENOMEM, CC_ERROR};
use libc::{c_char, c_uint, FILE};

use std::ffi::CStr;
use std::io::BufReader;

use crate::compat::CFileRef;

const CC_OK: rstatus_i = ccommon_sys::CC_OK as rstatus_i;

/// Set an option using the value parsed from the string.
///
/// Returns 0 for success, otherwise prints an error message
/// and returns an error code.
#[no_mangle]
unsafe extern "C" fn option_set(opt: *mut option, val_str: *const c_char) -> rstatus_i {
    use crate::option::ParseErrorKind;

    assert!(!val_str.is_null());
    assert!(!opt.is_null());

    let s = CStr::from_ptr(val_str);
    let bytes = s.to_bytes();

    match crate::option::option_set(&mut *opt, bytes) {
        Ok(()) => CC_OK,
        Err(e) => {
            eprintln!("Failed to parse option value: {}", e);

            match e.kind() {
                ParseErrorKind::OutOfMemory => CC_ENOMEM,
                _ => CC_ERROR,
            }
        }
    }
}

/// Initialize an option to it's default value.
///
/// Returns `0` for success, otherise prints an error message
/// and returns `CC_ENOMEM`.
///
/// Note: The only case where an error can occur here is when
///       we fail to allocate memory for a string.
#[no_mangle]
unsafe extern "C" fn option_default(opt: *mut option) -> rstatus_i {
    assert!(!opt.is_null());

    match crate::option::option_default(&mut *opt) {
        Ok(()) => CC_OK,
        Err(_) => {
            // This seems unlikely to work if we ran out of memory
            // while trying to set an option. Myabe it was a really
            // big string?
            eprintln!("Ran out of memory while attempting to set option to default");

            CC_ENOMEM
        }
    }
}

/// Print a single option stdout.
///
/// Will panic if printing to stdout fails.
#[no_mangle]
unsafe extern "C" fn option_print(opt: *const option) {
    assert!(!opt.is_null());

    crate::option::option_print(&mut std::io::stdout(), &*opt).expect("Failed to write to stdout");
}

/// Print all options in an array.
///
/// Will panic if printing to stdout fails.
#[no_mangle]
unsafe extern "C" fn option_print_all(opts: *const option, nopt: c_uint) {
    let slice = std::slice::from_raw_parts(opts, nopt as usize);

    crate::option::option_print_all(&mut std::io::stdout(), slice)
        .expect("Failed to write to stdout");
}

/// Print a description of all options in an array.
///
/// Will panic if printing to stdout fails.
#[no_mangle]
unsafe extern "C" fn option_describe_all(opts: *const option, nopt: c_uint) {
    let slice = std::slice::from_raw_parts(opts, nopt as usize);

    crate::option::option_describe_all(&mut std::io::stdout(), slice)
        .expect("Failed to write to stdout");
}

/// Initialize an array of options to the default values.
///
/// This returns `CC_OK` on success and `CC_ENOMEM` if it
/// failed to allocate memory for a string copy.
#[no_mangle]
unsafe extern "C" fn option_load_default(opts: *mut option, nopt: c_uint) -> rstatus_i {
    let slice = std::slice::from_raw_parts_mut(opts, nopt as usize);

    match crate::option::option_load_default(slice) {
        Ok(()) => CC_OK,
        Err(_) => CC_ENOMEM,
    }
}

/// Parse options in `.ini` format from a file.
///
/// Returns 0 on success. On error prints out an error message
/// and returns a non-zero error code.
#[no_mangle]
unsafe extern "C" fn option_load_file(fp: *mut FILE, opts: *mut option, nopt: c_uint) -> rstatus_i {
    use crate::option::ParseErrorKind;

    let slice = std::slice::from_raw_parts_mut(opts, nopt as usize);
    let file = CFileRef::from_ptr_mut(fp);
    let mut file = BufReader::new(file);

    match crate::option::option_load(slice, &mut file) {
        Ok(()) => CC_OK,
        Err(e) => {
            eprintln!("Failed to load options from file: {}", e);

            match e.kind() {
                ParseErrorKind::OutOfMemory => CC_ENOMEM,
                _ => CC_ERROR,
            }
        }
    }
}
