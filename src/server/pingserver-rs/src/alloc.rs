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

use std::alloc::{GlobalAlloc, Layout};
use std::os::raw::{c_int, c_void};

use ccommon_sys::{_cc_alloc, _cc_free};

use std::alloc::System;

macro_rules! c_str {
    ($s:expr) => {
        concat!($s, "\0").as_ptr() as *const i8
    };
}

/// Allocator using cc_alloc and cc_free to track and log allocations
pub struct LoggedAlloc;

const SYS_ALLOC: System = System;

unsafe impl GlobalAlloc for LoggedAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if layout.align() > 16 {
            // We can't communicate alignment to the ccommon allocator
            // so allocations with more strict alignment requirements
            // will need to go through the system allocator.
            //
            // The end result of this is that we don't log allocations
            // with alignment > 16.
            //
            // TODO: We should fix ccommon so that it can handle allocations
            //       with arbitrary (within reason) alignments.
            return SYS_ALLOC.alloc(layout);
        }

        _cc_alloc(layout.size(), c_str!(module_path!()), line!() as c_int) as *mut u8
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if layout.align() > 16 {
            return SYS_ALLOC.dealloc(ptr, layout);
        }

        _cc_free(ptr as *mut c_void, c_str!(module_path!()), line!() as c_int)
    }
}
