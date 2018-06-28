#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

#[macro_use]
extern crate log;
extern crate cdb_rs;
extern crate env_logger;
extern crate libc;

use std::ffi::CStr;
use std::os::raw::c_char;
use std::{slice, ptr};

use cdb_rs::{Result, CDB};

#[repr(C)]
pub struct CDBBString {
    len: u32,
    data: *const u8,
}

#[repr(C)]
pub struct CDBHandle {
    inner: Box<CDB>,
}

fn mk_cdb_handler(path: String) -> Result<CDBHandle> {
    assert!(!path.is_empty(), "cdb file path was empty, misconfiguration?");
    debug!("mk_cdb_handler, path: {:?}", path);
    let cdbr = CDB::load(&path)?;
    debug!("cdbr: {:?}", cdbr);

    let inner = Box::new(cdbr);
    let handle = CDBHandle { inner };
    Ok(handle)
}

fn cstr_to_string(s: *const c_char) -> Result<String> {
    let ps = unsafe { CStr::from_ptr(s) }.to_str()?;
    let rv = String::from(ps);
    eprintln!("cstr_to_string: {:?}", rv);

    Ok(rv)
}

#[no_mangle]
pub extern "C" fn cdb_handle_create(path: *const c_char) -> *mut CDBHandle {
    assert!(!path.is_null());

    match cstr_to_string(path).and_then(|s| mk_cdb_handler(s)) {
        Ok(bhandle) => {
            Box::into_raw(Box::new(bhandle))
        }
        Err(err) => {
            error!("failed to create CDBHandle: {:?}", err);
            ptr::null_mut()
        }
    }
}

// the caller must call cdb_bstring_destroy with the returned (non-NULL) pointer when finished
// to ensure memory is freed.
//
// this _h variant means that you pass an explicit handle in, rather than using the HANDLE
#[no_mangle]
pub extern "C" fn cdb_get(h: *mut CDBHandle, k: *const CDBBString) -> Option<*const CDBBString> {
    assert!(!h.is_null());
    assert!(!k.is_null());

    let handle = unsafe { &*h };
    let k_bstr = unsafe { &*k };

    let mut buf = Vec::new();

    let key = unsafe { slice::from_raw_parts(k_bstr.data, k_bstr.len as usize) };

    match handle.inner.get(&key[..], &mut buf) {
        Err(err) => {
            error!("get failed with error: {:?}", err);
            None
        }
        Ok(Some(_)) => {
            let rsp = Box::new(CDBBString {
                len: buf.len() as u32,
                data: buf.as_ptr(),
            });
            Some(Box::into_raw(rsp))
        }
        Ok(None) => None,
    }
}

#[no_mangle]
pub extern "C" fn cdb_bstring_destroy(v: *mut CDBBString) {
    unsafe {
        drop(Box::from_raw(v));
    }
}

#[no_mangle]
pub extern "C" fn cdb_handle_destroy(handle: *mut CDBHandle) {
    unsafe {
        drop(Box::from_raw(handle));
    }
}

#[no_mangle]
pub extern "C" fn cdb_setup() {
    env_logger::init();
    debug!("setup cdb");
}

#[no_mangle]
pub extern "C" fn cdb_teardown() {
    debug!("teardown cdb");
}
