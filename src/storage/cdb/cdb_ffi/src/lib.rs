#[macro_use]
extern crate log;
extern crate bytes;
extern crate cdb_rs;
extern crate env_logger;
extern crate libc;

extern crate cdb_ccommon;

mod ccommon;

use std::convert::From;
use std::ffi::CStr;
use std::os::raw::c_char;
use std::path::PathBuf;
use std::ptr;

use cdb_rs::cdb;
use cdb_rs::{Mmap, Result, CDB};

use ccommon::bstring::{BStringRef, BStringRefMut};
use cdb_ccommon::bindings as bind;

#[repr(C)]
pub enum CDBData {
    Boxed(Box<[u8]>),
    Mmapped(Mmap),
}

#[repr(C)]
pub struct CDBHandle {
    data: CDBData,
}

#[repr(C)]
pub enum CDBStoreMethod {
    HEAP = 1,
    MMAP = 2,
}

impl CDBHandle {
    pub unsafe fn from_raw<'a>(ptr: *mut CDBHandle) -> &'a CDBHandle {
        &*ptr
    }

    pub fn new(data: CDBData) -> CDBHandle {
        CDBHandle { data }
    }
}

impl AsRef<[u8]> for CDBHandle {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        match self.data {
            CDBData::Boxed(ref bx) => bx.as_ref(),
            CDBData::Mmapped(ref mm) => mm.as_ref(),
        }
    }
}

enum LoadOption {
    Heap(PathBuf),
    Mmap(PathBuf),
}

#[inline]
fn mk_cdb_handler(lo: LoadOption) -> Result<CDBHandle> {
    match lo {
        LoadOption::Heap(pb) => cdb::mmap_bytes_at_path(&pb)
            .map(|mm| CDBHandle::new(CDBData::Mmapped(mm))),
        LoadOption::Mmap(pb) => cdb::load_bytes_at_path(&pb)
            .map(|b| CDBHandle::new(CDBData::Boxed(b))),
    }
}

#[inline]
fn cstr_to_path_buf(s: *const c_char) -> Result<PathBuf> {
    let ps = unsafe { CStr::from_ptr(s) }.to_str()?;

    assert!(!ps.is_empty(), "cdb file path was empty, misconfiguration?");

    let rv = PathBuf::from(ps);
    eprintln!("cstr_to_string: {:?}", rv);

    Ok(rv)
}

#[no_mangle]
pub extern "C" fn cdb_handle_create(
    path: *const c_char,
    opt: CDBStoreMethod
) -> *mut CDBHandle {
    assert!(!path.is_null());

    cstr_to_path_buf(path)
        .and_then(|pathbuf| {
            mk_cdb_handler(
                match opt {
                    CDBStoreMethod::HEAP => LoadOption::Heap(pathbuf),
                    CDBStoreMethod::MMAP => LoadOption::Mmap(pathbuf),
                }
            )
        })
        .map(|h| Box::into_raw(Box::new(h)))
        .unwrap_or_else(|err| {
            error!("failed to create cdb_handle {:?}", err);
            ptr::null_mut()
        })
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

    match CDB::new(handle).get(&key, &mut val) {
        Ok(Some(n)) => {
            {
                // this provides access to the underlying struct fields
                // so we can modify the .len to be the actual number of bytes
                // in the value.
                let mut vstr = val.as_mut();
                vstr.len = n as u32;
            }
            val.into_raw() // consume BufStringRefMut and return the underlying raw pointer
        }
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
