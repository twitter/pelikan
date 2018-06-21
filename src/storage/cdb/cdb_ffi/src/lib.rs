extern crate cdb_rs;
#[macro_use] extern crate log;
extern crate env_logger;

use std::ffi::CStr;
use std::os::raw::c_char;
use std::ops::Deref;
use std::slice;

use cdb_rs::{CDB, Result};

#[repr(C)]
pub struct CDBBString {
    len: u32,
    data: *const u8,
}

#[repr(C)]
pub struct CDBHandle {
    inner: Box<CDB>
}

impl Deref for CDBHandle {
    type Target = CDB;

    fn deref(&self) -> &<Self as Deref>::Target {
        &*self.inner
    }
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
pub extern "C" fn cdb_get(h: *mut CDBHandle, k: *const CDBBString) -> Option<CDBBString> {
    assert!(!h.is_null());
    assert!(!k.is_null());

    let handle = unsafe { &*h };
    let k_bstr = unsafe { &*k };

    let mut buf = Vec::new();

    let key = unsafe {
        slice::from_raw_parts(k_bstr.data, k_bstr.len as usize)
    };

    match handle.get(&key[..], &mut buf) {
        Err(err) => {
            error!("get failed with error: {:?}", err);
            None
        },
        Ok(Some(_)) => Some(CDBBString{len: buf.len() as u32, data: buf.as_ptr()}),
        Ok(None) => None,
    }
}

#[no_mangle]
pub extern "C" fn cdb_destroy(handle: *mut CDBHandle) {
    unsafe {
        drop(Box::from_raw(handle));
    }
}
