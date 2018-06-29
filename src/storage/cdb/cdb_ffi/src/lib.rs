#[macro_use] extern crate log;
extern crate bytes;
extern crate cdb_rs;
extern crate env_logger;
extern crate libc;

extern crate cdb_ccommon;

use std::convert::From;
use std::ffi::CStr;
use std::os::raw::c_char;
use std::{ptr, slice};

use cdb_rs::cdb;
use cdb_rs::{Result, CDB};

use cdb_ccommon as bind;

#[repr(C)]
pub struct CDBHandle {
    inner: Box<[u8]>,
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
    let handle = unsafe { h.as_ref() }.unwrap();

    let kptr: &bind::bstring = unsafe { &*k };
    let vptr: &mut bind::bstring = unsafe { &mut *v };

    let key = unsafe {
        slice::from_raw_parts(
            kptr.data as *const _ as *const u8,  // cast *const i8 -> *const u8
            kptr.len as usize
        )
    };

    let mut val = unsafe {
        slice::from_raw_parts_mut(
            vptr.data as *mut _ as *mut u8,
            vptr.len as usize
        )
    };

    match CDB::from(handle).get(key, &mut val)  {
        Ok(Some(n)) => {
            vptr.len = n as u32;
            vptr
        },
        Ok(None) => ptr::null_mut(),
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
