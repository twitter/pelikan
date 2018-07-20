use cdb_ccommon::bindings as bind;

use std::slice;
use std::io;
use std::ops::{Deref, DerefMut};
use std::os::raw::c_char;
use std::convert::AsMut;

/// BStringRef provides a wrapper around a raw pointer to a cc_bstring. It's important to note that
/// this struct does not take ownership of the underlying pointer, nor does it free it when it's
/// dropped.
///
// see go/rust-newtype-pattern
pub struct BStringRef<'a> {
    inner: &'a bind::bstring,
}

impl<'a> BStringRef<'a> {
    pub unsafe fn from_raw(ptr: *const bind::bstring) -> Self {
        assert!(!ptr.is_null());
        let inner = &*ptr;
        BStringRef{inner}
    }

    #[allow(dead_code)]
    pub fn into_raw(self) -> *const bind::bstring {
        self.inner as *const bind::bstring
    }
}

impl<'a> Deref for BStringRef<'a> {
    type Target = [u8];

    fn deref(&self) -> &<Self as Deref>::Target {
        unsafe {
            slice::from_raw_parts(
                self.inner.data as *const c_char as *const u8,  // cast *const i8 -> *const u8
                self.inner.len as usize
            )
        }
    }
}


#[derive(Debug)]
pub struct BStringRefMut<'a> {
    inner: &'a mut bind::bstring,
}

impl<'a> BStringRefMut<'a> {
    pub unsafe fn from_raw(ptr: *mut bind::bstring) -> Self {
        assert!(!ptr.is_null());
        BStringRefMut{inner: &mut *ptr}
    }

    pub fn into_raw(self) -> *mut bind::bstring {
        self.inner as *mut bind::bstring
    }
}

impl<'a> Deref for BStringRefMut<'a> {
    type Target = [u8];

    fn deref(&self) -> &<Self as Deref>::Target {
        unsafe {
            slice::from_raw_parts(
                self.inner.data as *const c_char as *const u8,
                self.inner.len as usize
            )
        }
    }
}

impl<'a> DerefMut for BStringRefMut<'a> {
    fn deref_mut(&mut self) -> &mut <Self as Deref>::Target {
        unsafe {
            slice::from_raw_parts_mut(
                self.inner.data as *mut u8,
                self.inner.len as usize
            )
        }
    }
}

impl<'a> io::Write for BStringRefMut<'a> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        DerefMut::deref_mut(self).write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        DerefMut::deref_mut(self).flush()
    }
}

impl<'a> AsMut<bind::bstring> for BStringRefMut<'a> {
    fn as_mut(&mut self) -> &mut bind::bstring {
        self.inner
    }
}


#[cfg(test)]
mod test {
    use super::*;
    use std::ffi::CString;
    use std::boxed::Box;
    use std::io::Write;

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

    #[test]
    fn bstring_ref() {
        let s = "sea change";
        let bsp = BStringStr(s).into_raw();
        let bsr = unsafe { BStringRef::from_raw(bsp) };

        assert_eq!(&bsr[0..4], b"sea ");
        assert_eq!(&bsr[0..10], b"sea change");
        assert_eq!(&bsr[..], b"sea change");

        unsafe { BStringStr::free(bsp) };
    }

    #[test]
    fn bstring_ref_mut() {
        let s = "sea change";
        let bsp = BStringStr(s).into_raw();
        let mut bsr = unsafe { BStringRefMut::from_raw(bsp) };

        let d = vec![0u8, 1u8, 2u8];
        assert_eq!(d.len(), 3);

        {
            let mut buf: &mut [u8] = &mut bsr;
            let n = buf.write(&d).unwrap();
            assert_eq!(n, 3);
        }
        

        assert_eq!(&bsr[0..3], &d[0..3]);

        unsafe { BStringStr::free(bsp) };
    }
}
