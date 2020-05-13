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

use std::sync::atomic::{AtomicBool, Ordering};

use ccommon_sys::{debug_options_st, debug_setup, debug_teardown};

use crate::Error;

static DEBUG_INIT: AtomicBool = AtomicBool::new(false);

/// RAII Wrapper around the ccommon debug logger.
///
/// # Safety
/// It is unsafe to call `debug_setup` or `debug_teardown`
/// while an instance of DebugLogger exists.
///
/// If a debug logger has already been created using `debug_setup`
/// then use [`from_existing`](#fn.from_existing) to create a
/// `DebugLogger` instance wrapping it.
pub struct DebugLogger(());

impl DebugLogger {
    /// Create a new debug logger.
    ///
    /// If another debug logger is currently active then
    /// this will return `Error::EInval`, otherwise the
    /// error status reflects the return code of `debug_setup`.
    pub fn new<'a, I>(opts: I) -> Result<Self, Error>
    where
        I: Into<Option<&'a debug_options_st>>,
    {
        if DEBUG_INIT
            .compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed)
            .is_err()
        {
            return Err(Error::EInval);
        }

        let opts = opts
            .into()
            .map(|x| x as *const _ as *mut _)
            .unwrap_or(std::ptr::null_mut());

        let status = unsafe { debug_setup(opts) };

        if status < 0 {
            Err(status.into())
        } else {
            Ok(Self(()))
        }
    }

    /// Overwrite the current debug logger with a new one.
    ///
    /// # Safety
    /// It is unsafe to call this method while logging is
    /// happening concurrently.
    pub unsafe fn overwrite<'a, I>(&self, opts: I) -> Result<Self, Error>
    where
        I: Into<Option<&'a debug_options_st>>,
    {
        let opts = opts
            .into()
            .map(|x| x as *const _ as *mut _)
            .unwrap_or(std::ptr::null_mut());

        let status = debug_setup(opts);

        if status < 0 {
            Err(status.into())
        } else {
            Ok(Self(()))
        }
    }

    /// Create a debug logger from one that has already been
    /// set up through `debug_setup` or via
    /// [`release`](#fn.release).
    ///
    /// # Safety
    /// It is unsafe to call this method while an existing
    /// instance of `DebugLogger` is live.
    pub unsafe fn from_existing() -> Self {
        DEBUG_INIT.store(true, Ordering::Relaxed);

        Self(())
    }

    /// Release the current global debug logger from the lifetime
    /// of this particular `DebugLogger`.
    ///
    /// # Safety
    /// It is unsafe to create a new debug logger before
    /// `debug_teardown` is called.
    pub unsafe fn release(self) {
        std::mem::forget(self);
        DEBUG_INIT.store(false, Ordering::Relaxed);
    }
}

impl Drop for DebugLogger {
    fn drop(&mut self) {
        unsafe {
            debug_teardown();
            DEBUG_INIT.store(false, Ordering::Relaxed);
        }
    }
}
