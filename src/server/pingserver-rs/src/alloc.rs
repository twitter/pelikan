use std::alloc::{GlobalAlloc, Layout};
use std::os::raw::{c_int, c_void};
use std::ptr::null_mut;

use ccommon_sys::{_cc_alloc, _cc_free};

macro_rules! c_str {
    ($s:expr) => {
        concat!($s, "\0").as_ptr() as *const i8
    };
}

/// Allocator using cc_alloc and cc_free to track and log allocations
pub struct LoggedAlloc;

unsafe impl GlobalAlloc for LoggedAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if layout.align() > 16 {
            return null_mut();
        }

        _cc_alloc(layout.size(), c_str!(module_path!()), line!() as c_int) as *mut u8
    }
    unsafe fn dealloc(&self, ptr: *mut u8, _: Layout) {
        _cc_free(ptr as *mut c_void, c_str!(module_path!()), line!() as c_int)
    }
}
