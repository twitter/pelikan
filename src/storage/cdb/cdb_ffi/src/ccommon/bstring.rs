use cdb_ccommon::bindings as bind;

use std::slice;
use std::boxed::Box;
use std::io;
use std::ffi::CString;
use std::ops::{Deref, DerefMut};
use std::convert::AsMut;

/// BStringRef provides a wrapper around a raw pointer to a cc_bstring. It's important to note that
/// this struct does not take ownership of the underlying pointer, nor does it free it when it's
/// dropped.
///
// see go/rust-newtype-pattern
pub struct BStringRef {
    ptr: *const bind::bstring,
}

impl BStringRef {
    pub fn from_raw(ptr: *const bind::bstring) -> Self {
        assert!(!ptr.is_null());
        BStringRef{ptr}
    }

    pub fn into_raw(self) -> *const bind::bstring {
        self.ptr
    }
}

/// Allows one to call a function
impl Deref for BStringRef {
    type Target = [u8];

    fn deref(&self) -> &<Self as Deref>::Target {
        unsafe {
            let bs = &*self.ptr;
            slice::from_raw_parts(
                bs.data as *const _ as *const u8,  // cast *const i8 -> *const u8
                bs.len as usize
            )
        }
    }
}

struct BStringStr<'a>(&'a str);

impl<'a> BStringStr<'a> {
    fn into_raw(self) -> *mut bind::bstring {
        let bs = bind::bstring{
            len: self.0.len() as u32,
            data: CString::new(self.0).unwrap().into_raw(),
        };

        Box::into_raw(Box::new(bs))
    }

    /// Frees a BStringStr that was previously converted into a *mut bind::bstring via the
    /// into_raw method. Passing this method a pointer created through other means
    /// may lead to undefined behavior.
    unsafe fn free(ptr: *mut bind::bstring) {
        let b: Box<bind::bstring> = Box::from_raw(ptr);
        // reclaim pointer from the bstring, allowing it to be freed
        let _x = CString::from_raw(b.data);
    }
}

mod test {
    use super::*;

    #[test]
    fn bstring_ref_borrow() {
        let s = "sea change";
        let bsp = BStringStr(s).into_raw();

        let bsr = BStringRef::from_raw(bsp);

        unsafe { BStringStr::free(bsp) };
    }
}


pub struct BStringRefMut {
    ptr: *mut bind::bstring,
}

impl BStringRefMut {
    pub fn from_raw(ptr: *mut bind::bstring) -> Self {
        assert!(!ptr.is_null());
        BStringRefMut{ptr}
    }

    pub fn into_raw(self) -> *mut bind::bstring {
        self.ptr
    }
}

impl Deref for BStringRefMut {
    type Target = [u8];

    fn deref(&self) -> &<Self as Deref>::Target {
        unsafe {
            let bs = &*self.ptr;
            slice::from_raw_parts(
                bs.data as *const _ as *const u8,  // cast *mut i8 -> *const u8
                bs.len as usize
            )
        }
    }
}

impl DerefMut for BStringRefMut {
    fn deref_mut(&mut self) -> &mut <Self as Deref>::Target {
        unsafe {
            let bs = &*self.ptr;
            slice::from_raw_parts_mut(
                bs.data as *mut _ as *mut u8,  // cast *mut i8 -> *const u8
                bs.len as usize
            )
        }
    }
}

impl io::Write for BStringRefMut {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        DerefMut::deref_mut(self).write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        DerefMut::deref_mut(self).flush()
    }
}

impl AsMut<bind::bstring> for BStringRefMut {
    fn as_mut(&mut self) -> &mut bind::bstring {
        unsafe { &mut *self.ptr }
    }
}
