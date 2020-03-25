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

// gag doesn't work on windows
#![cfg(not(windows))]

#[macro_use]
extern crate log;
#[macro_use]
extern crate rusty_fork;

use ccommon_rs::log::*;

use std::io::Read;
use std::os::raw::c_char;

macro_rules! c_str {
    ($s:expr) => {
        #[allow(unused_unsafe)]
        unsafe {
            std::ffi::CStr::from_bytes_with_nul_unchecked(concat!($s, "\0").as_bytes())
        }
    };
}

pub fn capture_stderr<F: FnOnce() -> ()>(func: F) -> Result<String, std::io::Error> {
    let mut buf = gag::BufferRedirect::stderr()?;

    func();

    let mut output = String::new();
    buf.read_to_string(&mut output)?;

    Ok(output)
}

fn test_log_printing() {
    let _logger = DebugLogger::new(None);
    ccommon_rs::log::init().unwrap();

    let output = capture_stderr(|| {
        error!("<== TEST ERROR ==>");
    })
    .unwrap();

    eprintln!("'{}'", output);
    assert!(output.contains("<== TEST ERROR ==>"));
}

fn test_log_levels() {
    use cc_binding::*;
    use ccommon_rs::option::Options;

    let mut opts = debug_options_st::new();
    unsafe {
        // Set log level to debug
        option_set(
            &mut opts.debug_log_level as *mut _,
            c_str!("5").as_ptr() as *const c_char as *mut _,
        );
    }

    let _logger = DebugLogger::new(&opts);
    ccommon_rs::log::init().unwrap();

    let output = capture_stderr(|| {
        error!("<== ERROR ==>");
        warn!("<== WARN ==>");
        info!("<== INFO ==>");
        debug!("<== DEBUG ==>");
        trace!("<== TRACE ==>");
    })
    .unwrap();

    assert!(output.contains("<== ERROR ==>"));
    assert!(output.contains("<== WARN ==>"));
    assert!(output.contains("<== INFO ==>"));
    assert!(output.contains("<== DEBUG ==>"));
    assert!(!output.contains("<== TRACE ==>"));
}

fn test_teardown() {
    ccommon_rs::log::init().unwrap();

    let output = capture_stderr(|| {
        info!("<< LOG 1 >>");

        {
            let _logger = DebugLogger::new(None);

            info!("<< LOG 2 >>");
            info!("<< LOG 3 >>");
        }

        info!("<< LOG 4 >>");

        {
            let _logger = DebugLogger::new(None);

            info!("<< LOG 5 >>");
        }

        info!("<< LOG 6 >>");
    })
    .unwrap();

    assert!(!output.contains("<< LOG 1 >>"));
    assert!(output.contains("<< LOG 2 >>"));
    assert!(output.contains("<< LOG 3 >>"));
    assert!(!output.contains("<< LOG 4 >>"));
    assert!(output.contains("<< LOG 5 >>"));
    assert!(!output.contains("<< LOG 6 >>"));
}

rusty_fork_test! {
    #[test]
    fn log_printing() { test_log_printing() }
}

rusty_fork_test! {
    #[test]
    fn log_levels() { test_log_levels() }
}

rusty_fork_test! {
    #[test]
    fn teardown() { test_teardown() }
}
