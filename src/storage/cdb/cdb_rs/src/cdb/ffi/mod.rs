use cc_binding as bind;
use ccommon_rs::bstring::BStr;

use cdb::{self, cdb_handle, Reader, Result};

use env_logger; // TODO: switch to cc_log_rs

use std::convert::From;
use std::ffi::CStr;
use std::os::raw::c_char;
use std::ptr;

pub(super) mod gen;

fn mk_cdb_handler(path: &str) -> Result<cdb_handle> {
    assert!(
        !path.is_empty(),
        "cdb file path was empty, misconfiguration?"
    );
    debug!("mk_cdb_handler, path: {:?}", path);
    let inner = cdb::load_bytes_at_path(path)?;

    Ok(cdb_handle::new(inner))
}

fn cstr_to_string(s: *const c_char) -> Result<String> {
    let ps = unsafe { CStr::from_ptr(s) }.to_str()?;
    Ok(String::from(ps))
}

#[no_mangle]
pub extern "C" fn cdb_handle_create(path: *const c_char) -> *mut cdb_handle {
    assert!(!path.is_null());

    match cstr_to_string(path).and_then(|s| mk_cdb_handler(&s)) {
        Ok(bhandle) => Box::into_raw(Box::new(bhandle)),
        Err(err) => {
            error!("failed to create cdb_handle: {:?}", err);
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn cdb_get(
    h: *mut cdb_handle,
    k: *const bind::bstring,
    v: *mut bind::bstring,
) -> *mut bind::bstring {
    assert!(!h.is_null());
    assert!(!k.is_null());
    assert!(!v.is_null());

    // TODO: don't do unwrap, be safe
    let handle = cdb_handle::from_raw(h);
    let key = BStr::from_ptr(k as *mut _);
    let mut val = BStr::from_ptr_mut(v);

    match Reader::from(handle).get(&key, &mut val) {
        Ok(Some(n)) => {
            {
                // this provides access to the underlying struct fields
                // so we can modify the .len to be the actual number of bytes
                // in the value.
                let mut vstr = val.as_mut();
                vstr.len = n as u32;
            }
            val.as_ptr()
        }
        Ok(None) => ptr::null_mut(), // not found, return a NULL
        Err(err) => {
            eprintln!("got error: {:?}", err);
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn cdb_handle_destroy(handle: *mut *mut cdb_handle) {
    drop(Box::from_raw(*handle));
    *handle = ptr::null_mut()
}

#[no_mangle]
pub extern "C" fn cdb_setup() {
    env_logger::init();
    eprintln!("setup cdb");
}

#[no_mangle]
pub extern "C" fn cdb_teardown() {
    eprintln!("teardown cdb");
}

#[cfg(test)]
mod test {
    use super::*;
    use cdb::backend::Backend;
    use cdb::cdb_handle;

    #[test]
    fn cdb_handle_destroy_should_null_out_the_passed_ptr() {
        let be = Backend::noop().unwrap();

        let handle = Box::new(cdb_handle::from(be));
        let mut p = Box::into_raw(handle) as *mut cdb_handle;

        let pp = (&mut p) as *mut *mut cdb_handle;
        unsafe { cdb_handle_destroy(pp) };
        assert!(p.is_null());
    }
}
