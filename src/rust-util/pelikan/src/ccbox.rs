
use std::pinned::Pin;
use std::ptr::NonNull;
use std::mem::{self, MaybeUninit};
use std::ops::*;

use ccommon_sys::{_cc_alloc, _cc_free};

macro_rules! c_str {
    ($s:expr) => {
        concat!($s, "\0").as_ptr() as *const i8
    }
}

#[repr(transparent)]
pub struct CCBox<T: ?Sized>(NonNull<T>);

unsafe impl<T: Send + ?Sized> Send for CCBox<T>{}
unsafe impl<T: Sync + ?Sized> Sync for CCBox<T>{}

impl<T> CCBox<T> {
    pub fn new(val: T) -> CCBox<T> {
        // Most malloc implementations give 16-byte alignment
        assert!(mem::align_of::<T>() <= 16);

        match Self::try_new(val) {
            Some(x) => x,
            None => panic!("Failed to allocate memory")
        }
    }

    pub fn try_new(val: T) -> Option<CCBox<T>> {
        if mem::size_of_val(&val) == 0 {
            return Some(CCBox(NonNull::dangling()));
        }

        if mem::align_of_val(&val) > 16 {
            return None;
        }

        let ptr = unsafe {
            _cc_alloc(
                mem::size_of_val(&val),
                c_str!(module_path!()),
                line!()
            ) as *mut MaybeUnint<T>
        };

        Some(CCBox(NonNull::new(ptr)?))
    }
}

impl<T: ?Sized> Box<T> {
    /// Construct a `CCBox` from a raw pointer.
    /// 
    /// After this function is called the raw pointer is owned by
    /// the resulting `CCBox`. When the box is dropped, it will run
    /// the destructor for the stored value and free the pointer
    /// using [`_cc_free`](ccommon_sys::_cc_free).
    /// 
    /// # Safety
    /// For this to be safe the pointer must have been allocated
    /// using [`_cc_alloc`](ccommon_sys::_cc_alloc). In addition,
    /// calling `CCBox::from_raw`
    pub unsafe fn from_raw(raw: *mut T) -> Self {
        assert!(!raw.is_null());

        CCBox(NonNull::new_unchecked(raw))
    }

    /// Comsumes the box and returns the pointer stored inside.
    /// 
    /// The pointer should be freed using [`_cc_free`](ccommon_sys::_cc_free)
    /// or by recreating a `CCBox` from the pointer.
    pub fn into_raw(b: Self) -> *mut T {
        let ptr = b.0.as_mut_ptr();
        mem::drop(b);
        ptr
    }
}

impl<T: ?Sized> Deref for CCBox<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &**self
    }
}

impl<T: ?Sized> DerefMut for CCBox<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut **self
    }
}

impl<T: Default> Default for CCBox<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}
