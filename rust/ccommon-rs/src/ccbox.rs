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

use std::convert::TryInto;
use std::fmt;
use std::mem::{self, MaybeUninit};
use std::ops::*;
use std::ptr::NonNull;

use ccommon_sys::{_cc_alloc, _cc_free};

use crate::error::AllocationError;

macro_rules! c_str {
    ($s:expr) => {
        concat!($s, "\0").as_ptr() as *const i8
    };
}

/// A box that allocates and frees using `cc_alloc`
/// and `cc_free`.
#[repr(transparent)]
pub struct CCBox<T: ?Sized>(NonNull<T>);

unsafe impl<T: Send + ?Sized> Send for CCBox<T> {}
unsafe impl<T: Sync + ?Sized> Sync for CCBox<T> {}

impl<T> CCBox<T> {
    /// Create a new `CCBox` with `val` inside.
    ///
    /// # Panics
    /// Panics if the underlying allocator fails to allocate
    /// or if `T` requires an alignment of greater than 16.
    pub fn new(val: T) -> CCBox<T> {
        // Most malloc implementations give 16-byte alignment
        assert!(mem::align_of::<T>() <= 16);

        match Self::try_new(val) {
            Ok(x) => x,
            Err(e) => panic!("{}", e),
        }
    }

    /// Attempt to create a new `CCBox` with `val` inside.
    ///
    /// Since the underlying allocator does not support
    /// passing alignments, any allocation with alignment
    /// greater than 16 will fail.
    ///
    /// In addition, returns an error whenever the underlying
    /// allocator returns `NULL`.
    pub fn try_new(val: T) -> Result<CCBox<T>, AllocationError<T>> {
        if mem::size_of_val(&val) == 0 {
            return Ok(CCBox(NonNull::dangling()));
        }

        if mem::align_of_val(&val) > 16 {
            return Err(AllocationError::new());
        }

        unsafe {
            let ptr = _cc_alloc(
                mem::size_of_val(&val).try_into().unwrap(),
                c_str!(module_path!()),
                line!() as std::os::raw::c_int,
            ) as *mut MaybeUninit<T>;

            *ptr = MaybeUninit::new(val);

            Ok(CCBox(match NonNull::new(ptr as *mut T) {
                Some(x) => x,
                None => return Err(AllocationError::new()),
            }))
        }
    }
}

impl<T: ?Sized> CCBox<T> {
    /// Construct a `CCBox` from a raw pointer.
    ///
    /// After this function is called the raw pointer is owned by
    /// the resulting `CCBox`. When the box is dropped, it will run
    /// the destructor for the stored value and free the pointer
    /// using [`_cc_free`](ccommon_sys::_cc_free).
    ///
    /// # Safety
    /// For this to be safe the pointer must have been allocated
    /// using [`_cc_alloc`](ccommon_sys::_cc_alloc) (this includes
    /// values allocated through `CCBox::new`).
    pub unsafe fn from_raw(raw: *mut T) -> Self {
        assert!(!raw.is_null());

        CCBox(NonNull::new_unchecked(raw))
    }

    /// Consumes the box and returns the pointer stored inside.
    ///
    /// The pointer should be freed using [`_cc_free`](ccommon_sys::_cc_free)
    /// or by recreating a `CCBox` from the pointer.
    pub fn into_raw(b: Self) -> *mut T {
        let ptr = b.0.as_ptr();
        // Don't want to free the pointer
        mem::forget(b);
        ptr
    }
}

impl<T: ?Sized> Drop for CCBox<T> {
    fn drop(&mut self) {
        use std::ptr;

        if mem::size_of_val(&**self) != 0 {
            unsafe {
                ptr::drop_in_place(self.0.as_ptr());

                _cc_free(
                    self.0.as_ptr() as *mut std::ffi::c_void,
                    c_str!(module_path!()),
                    line!() as std::os::raw::c_int,
                );
            }
        }
    }
}

impl<T: ?Sized> Deref for CCBox<T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { self.0.as_ref() }
    }
}

impl<T: ?Sized> DerefMut for CCBox<T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { self.0.as_mut() }
    }
}

impl<T: Default> Default for CCBox<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T: ?Sized> AsRef<T> for CCBox<T> {
    fn as_ref(&self) -> &T {
        &*self
    }
}

impl<T: ?Sized> AsMut<T> for CCBox<T> {
    fn as_mut(&mut self) -> &mut T {
        &mut *self
    }
}

impl<T: Clone> Clone for CCBox<T> {
    fn clone(&self) -> Self {
        CCBox::new(self.as_ref().clone())
    }
}

impl<T: fmt::Debug + ?Sized> fmt::Debug for CCBox<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        (**self).fmt(fmt)
    }
}

impl<T: fmt::Display + ?Sized> fmt::Display for CCBox<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        (**self).fmt(fmt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ccbox_size_tests() {
        use std::mem::size_of;

        assert_eq!(size_of::<CCBox<()>>(), size_of::<*mut ()>());

        assert_eq!(size_of::<Option<CCBox<()>>>(), size_of::<*mut ()>());
    }

    #[test]
    fn ptr_round_trip() {
        unsafe {
            let ptr1 = _cc_alloc(16, c_str!("test"), 0);
            let boxed = CCBox::from_raw(ptr1);
            let ptr2 = CCBox::into_raw(boxed);

            // Ensure the pointer is properly freed
            let _boxed = CCBox::from_raw(ptr2);

            assert_eq!(ptr1, ptr2);
        }
    }

    #[test]
    fn overaligned() {
        #[repr(align(128))]
        struct OverAligned(u8);

        assert!(CCBox::try_new(OverAligned(0)).is_err());
    }
}
