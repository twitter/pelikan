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

//! Bindings related to logging.
//!
//! There are currently two parts to this module.
//! 1. `DebugLogger`: A RAII wrapper around ccommon's debug module. It is
//!    needed to set up the debug logging infrastructure that is used
//!    as a backend by
//! 2. The `log` shim. This is a logger that forwards the logging macros
//!    provided by the [`log`](https://docs.rs/log) crate into the debug
//!    logger initialized by part 1.
//! 3. A panic handler which will utilize ccommon's logging infrastructure
//!    in addition to printing out normal log messages.
//!
//! To use this module initialize the logging shim by calling [`init()`][0]
//! (alternatively, use [`rust_log_setup`][1] and [`rust_log_teardown`][2] from
//! C code). In addition, create an instance of [`DebugLogger`][3] (or
//! initialize it from C code).
//!
//! Now, just use the log macros as normal and all logs will be run through
//! ccommon's logging infrastructure.
//!
//! An optional extra thing that can be down is to use
//! [`set_panic_handler`][4] to set a panic hook which
//! will utilize common's logging infrastructure in addition to printing
//! to stderr.
//!
//! [0]: ccommon_rs::log::init
//! [1]: ccommon_rs::log::rust_log_setup
//! [2]: ccommon_rs::log::rust_log_teardown
//! [3]: ccommon_rs::log::DebugLogger
//! [4]: ccommon_rs::log::set_panic_handler

// Backend for the standard logging shim
mod debug;
mod shim;

pub use self::debug::DebugLogger;
pub use self::shim::{init, rust_log_setup, rust_log_teardown, set_panic_handler};
