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

use std::io::Write;
use std::os::raw::{c_char, c_int};
use std::panic::{self, PanicInfo};
use std::sync::atomic::{AtomicPtr, Ordering};
use std::thread;

use cc_binding::{_log, debug_log_flush, debug_logger, LOG_MAX_LEN};
use log::{Level, Log, Metadata, Record, SetLoggerError};

fn ccommon_dlog() -> &'static AtomicPtr<debug_logger> {
    unsafe { &*(&cc_binding::dlog as *const _ as *const AtomicPtr<debug_logger>) }
}

const FILENAME_MAX_LEN: usize = 1024;

/// A log implementation which wraps the debug logger from ccommon
struct CCLog;

unsafe impl Send for CCLog {}

unsafe impl Sync for CCLog {}

fn level_to_int(level: Level) -> c_int {
    use cc_binding::*;

    let level = match level {
        Level::Trace => LOG_VERB,
        Level::Debug => LOG_DEBUG,
        Level::Info => LOG_INFO,
        Level::Warn => LOG_WARN,
        Level::Error => LOG_ERROR,
    };

    level as c_int
}

impl CCLog {
    fn enabled_ptr(metadata: &Metadata, ptr: *const debug_logger) -> bool {
        if ptr.is_null() {
            return false;
        }

        unsafe {
            // Note: Assumes the level doesn't change concurrently
            (*ptr).level >= level_to_int(metadata.level())
        }
    }
}

impl Log for CCLog {
    #[inline]
    fn enabled(&self, metadata: &Metadata) -> bool {
        Self::enabled_ptr(metadata, ccommon_dlog().load(Ordering::Relaxed))
    }

    fn log(&self, record: &Record) {
        let ptr = ccommon_dlog().load(Ordering::Relaxed);

        if !Self::enabled_ptr(record.metadata(), ptr) {
            return;
        }

        // +1 bytes are trailing nul bytes
        let mut buffer = [0; LOG_MAX_LEN as usize + 1];
        let mut filename = [0; FILENAME_MAX_LEN + 1];

        let _ = write!(&mut buffer[0..LOG_MAX_LEN as usize], "{}", record.args());

        // Make a best-effort attempt to provide a meainingful
        // location that is consistent with how ccommon does
        // log messages.
        let filestr = record
            .file()
            .or_else(|| record.module_path())
            .unwrap_or_else(|| record.target());
        let _ = write!(&mut filename[0..FILENAME_MAX_LEN], "{}", filestr);

        unsafe {
            _log(
                ptr,
                (&filename).as_ptr() as *const c_char,
                record.line().map(|x| x as c_int).unwrap_or(-1),
                level_to_int(record.level()),
                "%s\0".as_ptr() as *const c_char,
                (&buffer).as_ptr() as *const c_char,
            )
        }
    }

    fn flush(&self) {
        unsafe {
            // The argument here is unused and ccommon uses it for
            // compatibility with other modules such as timer_wheel
            debug_log_flush(std::ptr::null_mut());
        }
    }
}

// The logger has no state so a global zero-sized instance
// works just fine.
static LOGGER: CCLog = CCLog;

/// Initialize the logger. It will automatically use the
/// ccommon debug logger once that is set up.
pub fn init() -> Result<(), SetLoggerError> {
    use log::LevelFilter;

    // TODO: dynamically set this based on dlog?
    log::set_max_level(LevelFilter::Trace);

    log::set_logger(&LOGGER)
}

/// Set the global panic handler to one which logs the
/// panic at `LOG_CRIT` level before calling the original
/// panic hook.
pub fn set_panic_handler() {
    use cc_binding::LOG_CRIT;

    use std::env;
    use std::io::Cursor;

    // After logging the message we want to call whatever existing
    // panic hook was in place. In most cases this should be the
    // default hook.
    let old_hook = panic::take_hook();

    panic::set_hook(Box::new(move |info: &PanicInfo| {
        let mut buffer = [0u8; LOG_MAX_LEN as usize + 1];
        let ptr = ccommon_dlog().load(Ordering::Relaxed);

        let msg = if let Some(s) = info.payload().downcast_ref::<&str>() {
            *s
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            &s[..]
        } else {
            "Box<Any>"
        };

        // Cursor so that we can get the number of bytes written afterwards
        let mut cursor = Cursor::new(&mut buffer[0..LOG_MAX_LEN as usize]);

        // If the thread has a name then use it. (This mirrors std's standard
        // debug handler)
        let current_thread = thread::current();
        let name = current_thread.name().unwrap_or("<unnamed>");

        if let Some(ref location) = info.location() {
            let _ = write!(
                &mut cursor,
                "thread '{}' panicked at '{}', {}",
                name, msg, *location
            );
        } else {
            let _ = write!(&mut cursor, "thread '{}' panicked at '{}'", name, msg);
        };

        if !ptr.is_null() {
            // Ensure that the panic message is logged
            // TODO(sean): Do we want to log the backtrace as well?
            if let Some(location) = info.location() {
                unsafe {
                    _log(
                        ptr,
                        location.file().as_ptr() as *const c_char,
                        location.line() as c_int,
                        LOG_CRIT as c_int,
                        (&buffer).as_ptr() as *const c_char,
                    );
                }
            } else {
                unsafe {
                    _log(
                        ptr,
                        file!().as_ptr() as *const c_char,
                        line!() as c_int,
                        LOG_CRIT as c_int,
                        (&buffer).as_ptr() as *const c_char,
                    )
                }
            }
        }

        let old_val = env::var_os("RUST_BACKTRACE");
        // If possible, have the stdlib display a backtrace
        env::set_var("RUST_BACKTRACE", "full");
        old_hook(info);

        if let Some(var) = old_val {
            env::set_var("RUST_BACKTRACE", var);
        }
    }))
}

//================================================
// C Interface Methods
//================================================

#[no_mangle]
pub extern "C" fn rust_log_setup() -> cc_binding::rstatus_i {
    use cc_binding::*;

    match init() {
        Ok(()) => CC_OK as rstatus_i,
        Err(_) => CC_ERROR,
    }
}

#[no_mangle]
pub extern "C" fn rust_log_teardown() {
    // Provided for symmetry with the rest of ccommon, currently a no-op
}
