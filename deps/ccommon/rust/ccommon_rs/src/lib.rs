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

extern crate cc_binding;
extern crate chrono;
extern crate crossbeam;
#[macro_use]
extern crate failure;
#[macro_use]
extern crate failure_derive;
extern crate lazy_static;
#[macro_use]
extern crate log as rslog;
extern crate tempfile;
extern crate thread_id;
extern crate thread_local;
extern crate time;

#[cfg(test)]
#[macro_use]
extern crate rusty_fork;

use std::result;

pub mod bstring;
pub mod buf;
pub mod log;
pub mod util;

// like how guava provides enhancements for Int as "Ints"
pub mod ptrs;

pub type Result<T> = result::Result<T, failure::Error>;

/// Error codes that could be returned by ccommon functions.
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub enum Error {
    Generic = -1,
    EAgain = -2,
    ERetry = -3,
    ENoMem = -4,
    EEmpty = -5,
    ERdHup = -6,
    EInval = -7,
    EOther = -8,
}

impl From<std::os::raw::c_int> for Error {
    fn from(val: std::os::raw::c_int) -> Self {
        match val {
            -1 => Error::Generic,
            -2 => Error::EAgain,
            -3 => Error::ERetry,
            -4 => Error::ENoMem,
            -5 => Error::EEmpty,
            -6 => Error::ERdHup,
            -7 => Error::EInval,
            _ => Error::EOther,
        }
    }
}
