
use std::ffi::CStr;
use std::os::raw::c_char;

use super::{CDB, Result};

#[repr(C)]
pub struct BString {
    u32 len;

}

#[repr(C)]
pub struct CDBHandle {
    inner: Box<CDB>
}

#[no_mangle]
pub extern "C" fn cdb_create(path: *const c_char) -> Option<*mut CDBHandle> {
    assert!(!path.is_null());

    let f = || -> Result<Box<CDBHandle>> {
        let cpath = unsafe { CStr::from_ptr(path) };

        let path = cpath.to_str()?;
        let inner = Box::new(CDB::stdio(path)?);
        let handle = CDBHandle{inner};

        Ok(Box::new(handle))
    };

    match f() {
        Ok(bhandle) => Some(Box::into_raw(bhandle)),
        Err(err) => {
            error!("failed to create CDBHandle: {:?}", err);
            None
        }
    }
}

#[no_mangle]
pub extern "C" fn cdb_get(handle: *mut CDBHandle, key: *const u8) -> Option<*mut u8> {
    None
}

#[no_mangle]
pub extern "C" fn cdb_destroy(handle: *mut CDBHandle) {
    unsafe {
        drop(Box::from_raw(handle));
    }
}
