#[macro_use] extern crate log;
extern crate bytes;
extern crate cdb_rs;
extern crate env_logger;
extern crate libc;

extern crate cdb_ccommon;

mod ccommon;

use std::convert::From;
use std::ffi::CStr;
use std::os::raw::c_char;
use std::ptr;

use cdb_rs::cdb;
use cdb_rs::{Result, CDB};

use cdb_ccommon::bindings as bind;
use ccommon::bstring::{BStringRef, BStringRefMut};

#[repr(C)]
pub struct CDBHandle {
    inner: Box<[u8]>,
}

impl CDBHandle {
    pub unsafe fn from_raw<'a>(ptr: *mut CDBHandle) -> &'a CDBHandle { &*ptr }
}

impl<'a> From<&'a CDBHandle> for CDB<'a> {
    fn from(h: &'a CDBHandle) -> Self {
        CDB::new(&h.inner)
    }
}

fn mk_cdb_handler(path: String) -> Result<CDBHandle> {
    assert!(
        !path.is_empty(),
        "cdb file path was empty, misconfiguration?"
    );
    debug!("mk_cdb_handler, path: {:?}", path);
    let inner = cdb::load_bytes_at_path(path.as_ref())?;
    debug!("inner: {:?}", inner);

    Ok(CDBHandle { inner })
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
        Ok(bhandle) => Box::into_raw(Box::new(bhandle)),
        Err(err) => {
            error!("failed to create CDBHandle: {:?}", err);
            ptr::null_mut()
        }
    }
}


#[no_mangle]
pub extern "C" fn cdb_get(
    h: *mut CDBHandle,
    k: *const bind::bstring,
    v: *mut bind::bstring,
) -> *mut bind::bstring {
    assert!(!h.is_null());
    assert!(!k.is_null());
    assert!(!v.is_null());

    // TODO: don't do unwrap, be safe
    let handle = unsafe { CDBHandle::from_raw(h) };
    let key = unsafe { BStringRef::from_raw(k) };
    let mut val = unsafe { BStringRefMut::from_raw(v) };

    match CDB::from(handle).get(&key, &mut val)  {
        Ok(Some(n)) => {
            {
                // this provides access to the underlying struct fields
                // so we can modify the .len to be the actual number of bytes
                // in the value.
                let mut vstr = val.as_mut();
                vstr.len = n as u32;
            }
            val.into_raw()  // consume BufStringRefMut and return the underlying raw pointer
        },
        Ok(None) => ptr::null_mut(), // not found, return a NULL
        Err(err) => {
            eprintln!("got error: {:?}", err);
            ptr::null_mut()
        }
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
