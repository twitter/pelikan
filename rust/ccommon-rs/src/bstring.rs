// ccommon - a cache common library.
// Copyright (C) 2019 Twitter, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! BString is a wrapper around a foreign allocated and freed pointer to a cc_bstring.
//! It takes care of creating and freeing the foreign pointer within the normal
//! Rust lifetime rules. It has a companion reference object BStr, and the relation
//! of BString to BStr is similar to the relationship between String and &str.
//!
//! # Safety
//!
//! The point of this module is to ensure safe interaction between
//! Rust and C, and to facilitate passing BStrings between the two
//! as the sized buffer of choice.
//!
//! Much like with the standard library collections (Vec, Box), one
//! cannot simply pass their `from_raw` methods any old pointer.
//! You must only pass pointers obtained via the `into_raw` method.
//!
//! # Undefined Behavior
//!
//! Creating a BString from a Rust-allocated `bind::bstring` struct
//! will lead to undefined behavior if it is allowed to Drop. BString's
//! Drop implmentation passes the contained pointer to libc's free method
//! which can lead to memory corruption and [nasal demons].
//!
//! [nasal demons]: http://www.catb.org/jargon/html/N/nasal-demons.html

use std::borrow::Borrow;
use std::borrow::BorrowMut;
use std::boxed::Box;
use std::cell::UnsafeCell;
use std::fmt;
use std::fmt::Debug;
use std::fmt::Formatter;
use std::mem;
use std::ops::{Deref, DerefMut};
use std::slice;
use std::str::{self, Utf8Error};

pub type CCbstring = ccommon_sys::bstring;

#[doc(hidden)]
#[inline]
unsafe fn raw_ptr_to_bytes<'a>(ptr: *const CCbstring) -> &'a [u8] {
    slice::from_raw_parts((*ptr).data as *const _ as *const u8, (*ptr).len as usize)
}

#[doc(hidden)]
#[inline]
unsafe fn raw_ptr_to_bytes_mut<'a>(ptr: *mut CCbstring) -> &'a mut [u8] {
    slice::from_raw_parts_mut((*ptr).data as *mut _ as *mut u8, (*ptr).len as usize)
}

// this pattern lifted from https://docs.rs/foreign-types-shared/0.1.1/src/foreign_types_shared/lib.rs.html
struct Opaque(UnsafeCell<()>);

/// A reference to a BString. String is to &str as BString is to &BStr.
/// This should be used when one does not want to take ownership of the
/// underlying pointer, but wants to access it in a rust-friendly way.
///
/// This is useful in the case where the caller owns the buffer in the
/// data field and expects it to be filled (as opposed to in BString where
/// *we* own that memory).
///
pub struct BStr(Opaque);

impl BStr {
    /// Wraps a raw pointer to a cc_bstring struct with a BStr. This is a
    /// reference only conversion, and is zero cost.
    #[inline]
    pub unsafe fn from_ptr<'a>(ptr: *mut CCbstring) -> &'a Self {
        &*(ptr as *mut _)
    }

    /// Wraps a raw pointer to a cc_bstring struct with a BStr, and returns
    /// a mutable reference. This is a reference only conversion,
    /// and is zero cost.
    #[inline]
    pub unsafe fn from_ptr_mut<'a>(ptr: *mut CCbstring) -> &'a mut Self {
        &mut *(ptr as *mut _)
    }

    #[inline]
    pub fn as_ptr(&self) -> *mut CCbstring {
        self as *const _ as *mut _
    }

    pub fn from_ref(ccb: &CCbstring) -> &Self {
        unsafe { Self::from_ptr(ccb as *const CCbstring as *mut _) }
    }

    pub fn to_utf8_str(&self) -> Result<&str, Utf8Error> {
        str::from_utf8(&self[..])
    }

    pub fn to_utf8_string(&self) -> Result<String, Utf8Error> {
        self.to_utf8_str().map(|x| x.to_owned())
    }
}

impl Deref for BStr {
    type Target = [u8];

    #[inline]
    fn deref(&self) -> &[u8] {
        unsafe { raw_ptr_to_bytes(self.as_ptr()) }
    }
}

impl DerefMut for BStr {
    #[inline]
    fn deref_mut(&mut self) -> &mut [u8] {
        unsafe { raw_ptr_to_bytes_mut(self.as_ptr()) }
    }
}

impl AsRef<CCbstring> for BStr {
    fn as_ref(&self) -> &CCbstring {
        unsafe { &*self.as_ptr() }
    }
}

impl AsMut<CCbstring> for BStr {
    fn as_mut(&mut self) -> &mut CCbstring {
        unsafe { &mut *(self.as_ptr() as *mut _) }
    }
}

impl Borrow<CCbstring> for BStr {
    fn borrow(&self) -> &CCbstring {
        unsafe { &*self.as_ptr() }
    }
}

impl BorrowMut<CCbstring> for BStr {
    fn borrow_mut(&mut self) -> &mut CCbstring {
        unsafe { &mut *(self.as_ptr() as *mut _) }
    }
}

impl ToOwned for BStr {
    type Owned = BString;

    #[inline]
    fn to_owned(&self) -> BString {
        unsafe { BString::from_raw(self.as_ptr()).clone() }
    }
}

unsafe impl Send for BStr {}
unsafe impl Sync for BStr {}

/// An owned BString. By definition, a BString is allocated by
/// cc_bstring and freed by cc_bstring. This is because libc `malloc/free`
/// and Rust's `malloc/free` are two different implementations, and it's
/// important to keep track of who allocated what. To avoid this issue
/// all allocations are done from ccommon.
///
/// This struct has Drop defined, and will call `bstring_free` when its
/// dropped. You can avoid this by using the `into_raw` method which
/// essentially leaks the memory.
///
/// BString is safe for interoperation with ccommon libraries. You can pass
/// a pointer you got by using `BString::into_raw()` to C and everything will
/// just work.
///
/// # Examples
///
/// Creating and using a BString of a given size:
///
/// ```rust
/// # use ccommon_rs::bstring::*;
///
/// let mut bs = BString::new(3);
/// bs.copy_from_slice(&vec![0,1,2]);
///
/// assert_eq!(&bs[..], &[0,1,2])
/// ```
///
/// Creating an owned BString from Rust collections. If you can dereference it
/// to `&[u8]` you can copy it into a BString.
///
/// ```rust
/// # use ccommon_rs::bstring::*;
///
/// let s = BString::from("abc");
/// assert_eq!(&s[..], "abc".as_bytes());
///
/// let v = BString::from(vec![0, 1, 2]);
/// assert_eq!(&v[..], &[0, 1, 2]);
/// ```
///
/// Mutating the content of a BString:
///
/// ```rust
/// # use ccommon_rs::bstring::*;
///
/// let mut s = BString::from("abc");
/// s[0] = b'x';
/// s[1] = b'y';
/// s[2] = b'z';
/// assert_eq!(&s[..], "xyz".as_bytes());
/// ```
///
/// Use it as a buffer:
///
/// ```rust
/// # use ccommon_rs::bstring::*;
/// use std::io::*;
/// use std::str;
///
/// let mut x = BString::from("mutation is terrible ");
/// {
///     let mut c = Cursor::new(&mut x[..]);
///     let f = "fantastic".as_bytes();
///     c.seek(SeekFrom::End(-9));
///
///     let sz = c.write(&f[..]).unwrap();
///     assert_eq!(sz, f.len());
/// }
///
/// assert_eq!(
///     unsafe { str::from_utf8_unchecked(&x[..]) },
///     "mutation is fantastic"
/// );
/// ```
///
/// Note: if you're using BString as a buffer, it's important to
/// know that it *will not automatically resize*. If you write past the
/// end it will panic!.
pub struct BString(*mut CCbstring);

impl BString {
    pub fn new(size: u32) -> Self {
        let bsp: *mut CCbstring = unsafe { ccommon_sys::bstring_alloc(size) };

        assert!(!bsp.is_null());
        BString(bsp)
    }

    #[inline]
    pub fn into_raw(bs: BString) -> *mut CCbstring {
        let unique = bs.0;
        mem::forget(bs);
        unique
    }

    #[inline]
    pub unsafe fn from_raw(ptr: *mut CCbstring) -> BString {
        assert!(!ptr.is_null());
        BString(ptr)
    }

    /// Takes byte slice `&[u8]` and copies it into an owned BString.
    #[inline]
    pub fn from_bytes(s: &[u8]) -> Self {
        let bsp: *mut CCbstring = unsafe { ccommon_sys::bstring_alloc(s.len() as u32) };

        assert!(!bsp.is_null());

        let mut b = BString(bsp);
        b.as_bytes_mut().clone_from_slice(&s[..]);
        b
    }

    /// Copies the contents of `src` into self.
    ///
    /// # Panics
    ///
    /// This method will panic if `src.len() != self.len()`
    #[inline]
    // Note: Used for tests
    #[allow(dead_code)]
    fn copy_from_slice(&mut self, src: &[u8]) {
        assert_eq!(src.len(), self.len());
        (&mut (**self)).copy_from_slice(&src[..]);
    }

    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        unsafe { raw_ptr_to_bytes(self.0) }
    }

    #[inline]
    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        unsafe { raw_ptr_to_bytes_mut(self.0) }
    }

    #[inline]
    fn len(&self) -> usize {
        unsafe { (*self.0).len as usize }
    }

    pub fn to_utf8_str(&self) -> Result<&str, Utf8Error> {
        str::from_utf8(self.as_bytes())
    }

    pub fn to_utf8_string(&self) -> Result<String, Utf8Error> {
        self.to_utf8_str().map(|x| x.to_owned())
    }
}

impl Debug for BString {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        f.debug_struct("BString")
            .field("len", &self.len())
            .field("data", &self.as_bytes())
            .finish()
    }
}

impl PartialEq for BString {
    #[inline]
    fn eq(&self, other: &BString) -> bool {
        self.as_bytes().eq(other.as_bytes())
    }
}

impl Drop for BString {
    #[inline]
    fn drop(&mut self) {
        unsafe { ccommon_sys::bstring_free(&mut self.0) };
    }
}

impl Clone for BString {
    /// Create a copy of both the underlying struct _and_ the data it points to.
    #[inline]
    fn clone(&self) -> Self {
        BString::from_bytes(self.as_bytes())
    }
}

impl Deref for BString {
    type Target = BStr;

    #[inline]
    fn deref(&self) -> &BStr {
        unsafe { BStr::from_ptr(self.0) }
    }
}

impl DerefMut for BString {
    #[inline]
    fn deref_mut(&mut self) -> &mut BStr {
        unsafe { BStr::from_ptr_mut(self.0) }
    }
}

impl AsMut<BStr> for BString {
    fn as_mut(&mut self) -> &mut BStr {
        &mut (*self)
    }
}

impl AsRef<BStr> for BString {
    #[inline]
    fn as_ref(&self) -> &BStr {
        &*self
    }
}

impl Borrow<BStr> for BString {
    #[inline]
    fn borrow(&self) -> &BStr {
        &*self
    }
}

impl From<Vec<u8>> for BString {
    #[inline]
    fn from(v: Vec<u8>) -> Self {
        BString::from_bytes(&v[..])
    }
}

impl From<BString> for Vec<u8> {
    #[inline]
    fn from(bs: BString) -> Self {
        let mut v = Vec::with_capacity(bs.len());
        v.copy_from_slice(&**bs); // &**bs is &(BString -> BStr -> [u8])
        v
    }
}

impl From<Box<[u8]>> for BString {
    #[inline]
    fn from(b: Box<[u8]>) -> Self {
        BString::from_bytes(&b[..])
    }
}

impl<'a> From<&'a str> for BString {
    #[inline]
    fn from(s: &'a str) -> Self {
        BString::from_bytes(s.as_bytes())
    }
}

unsafe impl Send for BString {}
unsafe impl Sync for BString {}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_raw_ptr_to_bytes() {
        let bs = CCbstring {
            len: 5,
            data: "abcde".as_ptr() as *mut i8,
        };

        let ptr: *const CCbstring = &bs as *const CCbstring;

        let slice = unsafe { raw_ptr_to_bytes(ptr) };

        assert_eq!(slice.as_ptr(), bs.data as *mut u8);
        assert_eq!(slice.len(), 5);
        assert_eq!(slice, "abcde".as_bytes());
    }

    #[test]
    fn test_raw_ptr_to_bytes_mut() {
        let mut bs = BString::new(5);
        BString::copy_from_slice(&mut bs, "abcde".as_bytes());

        let ptr: *const CCbstring = BString::into_raw(bs) as *const CCbstring;

        {
            let s = unsafe { raw_ptr_to_bytes_mut(ptr as *mut _) };
            s[0] = 0;
        }

        let s = unsafe { raw_ptr_to_bytes(ptr) };
        assert_eq!(s[0], 0);
    }

    #[test]
    fn test_bstring_from_str() {
        let bs = BString::from("wat");

        assert_eq!(bs.as_bytes(), "wat".as_bytes());
    }

    #[test]
    fn test_bstring_into_raw_pointer_remains_valid() {
        let bsp: *mut CCbstring;
        {
            let mut bs = BString::new(5);
            bs[0] = 12u8;
            bsp = BString::into_raw(bs);
        }

        // the bsp pointer should still be valid here even though bs has been dropped
        let bytes = unsafe { raw_ptr_to_bytes(bsp) };
        assert_eq!(bytes[0], 12u8);
    }

    #[test]
    fn test_bstring_copy_from_slice() {
        let mut bs = BString::new(5);
        bs.copy_from_slice("abcde".as_bytes());
        assert_eq!(&bs[..], "abcde".as_bytes());
    }

    fn foreign_code(s: &str) -> *mut CCbstring {
        BString::into_raw(BString::from(s))
    }

    #[test]
    fn test_bstr_from_ptr() {
        let s = "abc";
        let ptr: *mut CCbstring = foreign_code(s);
        let bstr = unsafe { BStr::from_ptr(ptr) };
        assert_eq!(bstr.len(), 3);
        assert_eq!(&bstr[..], &s.as_bytes()[..]);

        unsafe { BString::from_raw(ptr) };
    }

    #[test]
    fn test_bstring_as_io_write() {
        use std::io::*;

        let mut x = BString::from("mutation is terrible ");
        {
            let mut c = Cursor::new(&mut x[..]);
            let f = "fantastic".as_bytes();
            c.seek(SeekFrom::End(-9)).unwrap();

            let sz = c.write(&f[..]).unwrap();
            assert_eq!(sz, f.len());
        }

        assert_eq!(
            unsafe { str::from_utf8_unchecked(&x[..]) },
            "mutation is fantastic"
        );
    }
}
