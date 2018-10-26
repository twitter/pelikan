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

use std::result;
use std::ptr;

#[derive(Fail, Debug)]
#[fail(display = "Null pointer exception")]
pub struct NullPointerError;

pub fn lift_to_option<T>(p: *mut T) -> Option<*mut T> {
    if p.is_null() {
        None
    } else {
        Some(p)
    }
}

pub fn null_check<T>(p: *mut T) -> result::Result<*mut T, NullPointerError> {
    lift_to_option(p).ok_or_else(|| NullPointerError)
}

pub fn opt_to_null_mut<T>(o: Option<*mut T>) -> *mut T {
    match o {
        Some(p) => p,
        None => ptr::null_mut(),
    }
}

