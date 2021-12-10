// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! A raw byte-level representation of an item.
//!
//! Unlike an [`Item`], the [`RawItem`] does not contain any fields which are
//! shared within a hash bucket such as the CAS value.

use crate::item::*;
use crate::SegError;
use std::convert::TryInto;

/// The raw byte-level representation of an item
#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct RawItem {
    data: *mut u8,
}

impl RawItem {
    /// Get an immutable borrow of the item's header
    pub(crate) fn header(&self) -> &ItemHeader {
        unsafe { &*(self.data as *const ItemHeader) }
    }

    /// Get a mutable borrow of the item's header
    pub(crate) fn header_mut(&mut self) -> *mut ItemHeader {
        self.data as *mut ItemHeader
    }

    /// Create a `RawItem` from a pointer
    ///
    /// # Safety
    ///
    /// Creating a `RawItem` from a pointer that does not point to a valid raw
    /// item or a pointer which is not 64bit aligned will result in undefined
    /// behavior. It is up to the caller to ensure that the item is constructed
    /// from a properly aligned pointer to valid data.
    pub(crate) fn from_ptr(ptr: *mut u8) -> RawItem {
        Self { data: ptr }
    }

    /// Returns the key length
    #[inline]
    pub(crate) fn klen(&self) -> u8 {
        self.header().klen()
    }

    /// Borrow the key
    pub(crate) fn key(&self) -> &[u8] {
        unsafe {
            let ptr = self.data.add(self.key_offset());
            let len = self.klen() as usize;
            std::slice::from_raw_parts(ptr, len)
        }
    }

    /// Returns the value length
    #[inline]
    pub(crate) fn vlen(&self) -> u32 {
        self.header().vlen()
    }

    pub fn value(&self) -> Value {
        if let Ok(v) = self.value_numeric() {
            Value {
                inner: TypedValue::U64(v),
            }
        } else {
            Value {
                inner: TypedValue::Bytes(self.value_bytes()),
            }
        }
    }

    /// Borrow the value
    // TODO(bmartin): should probably change this to be Option<>
    fn value_bytes(&self) -> &[u8] {
        unsafe {
            let ptr = self.data.add(self.value_offset());
            let len = self.vlen() as usize;
            std::slice::from_raw_parts(ptr, len)
        }
    }

    /// Get the value as an unsigned 64bit integer if it is numeric
    fn value_numeric(&self) -> Result<u64, SegError> {
        if self.header().is_num() && self.header().vlen() == 8 {
            Ok(u64::from_be_bytes(self.value_bytes().try_into().unwrap()))
        } else {
            Err(SegError::NotNumeric)
        }
    }

    /// Returns the optional data length
    #[inline]
    pub(crate) fn olen(&self) -> u8 {
        self.header().olen()
    }

    /// Borrow the optional data
    pub(crate) fn optional(&self) -> Option<&[u8]> {
        if self.olen() > 0 {
            unsafe {
                let ptr = self.data.add(self.optional_offset());
                let len = self.olen() as usize;
                Some(std::slice::from_raw_parts(ptr, len))
            }
        } else {
            None
        }
    }

    /// Check the header magic bytes
    #[inline]
    pub(crate) fn check_magic(&self) {
        self.header().check_magic()
    }

    /// Set the header magic bytes
    #[inline]
    pub(crate) fn set_magic(&mut self) {
        #[cfg(feature = "magic")]
        unsafe {
            (*self.header_mut()).set_magic()
        }
    }

    /// Copy data into the item
    pub(crate) fn define(&mut self, key: &[u8], value: &Value, optional: &[u8]) {
        unsafe {
            self.set_magic();
            (*self.header_mut()).set_deleted(false);
            (*self.header_mut()).set_olen(optional.len() as u8);
            std::ptr::copy_nonoverlapping(
                optional.as_ptr(),
                self.data.add(self.optional_offset()),
                optional.len(),
            );
            (*self.header_mut()).set_klen(key.len() as u8);
            std::ptr::copy_nonoverlapping(
                key.as_ptr(),
                self.data.add(self.key_offset()),
                key.len(),
            );
            (*self.header_mut()).set_vlen(value.packed_len() as u32);
            match value.inner {
                TypedValue::Bytes(value) => {
                    (*self.header_mut()).set_num(false);
                    std::ptr::copy_nonoverlapping(
                        value.as_ptr(),
                        self.data.add(self.value_offset()),
                        value.len(),
                    );
                }
                TypedValue::OwnedBytes(ref value) => {
                    (*self.header_mut()).set_num(false);
                    std::ptr::copy_nonoverlapping(
                        value.as_ptr(),
                        self.data.add(self.value_offset()),
                        value.len(),
                    );
                }
                TypedValue::U64(value) => {
                    let value = value.to_be_bytes().to_vec();
                    (*self.header_mut()).set_num(true);
                    std::ptr::copy_nonoverlapping(
                        value.as_ptr(),
                        self.data.add(self.value_offset()),
                        value.len(),
                    );
                }
            }
        }
    }

    // Gets the offset to the optional data
    #[inline]
    fn optional_offset(&self) -> usize {
        ITEM_HDR_SIZE
    }

    // Gets the offset to the key
    #[inline]
    fn key_offset(&self) -> usize {
        self.optional_offset() + self.olen() as usize
    }

    // Gets the offset to the value
    #[inline]
    fn value_offset(&self) -> usize {
        self.key_offset() + self.klen() as usize
    }

    /// Returns item size, rounded up for alignment
    pub(crate) fn size(&self) -> usize {
        (((ITEM_HDR_SIZE + self.olen() as usize + self.klen() as usize + self.vlen() as usize)
            >> 3)
            + 1)
            << 3
    }

    /// Sets the tombstone
    pub(crate) fn tombstone(&mut self) {
        unsafe { (*self.header_mut()).set_deleted(true) }
    }

    /// Checks if the item is deleted
    pub(crate) fn deleted(&self) -> bool {
        self.header().is_deleted()
    }

    pub(crate) fn increment(&self, rhs: u64) -> Result<u64, SegError> {
        let value = self.value_numeric()?;
        let value = value.wrapping_add(rhs);
        unsafe {
            std::ptr::copy_nonoverlapping(
                value.to_be_bytes().as_ptr(),
                self.data.add(self.value_offset()),
                core::mem::size_of::<u64>(),
            );
        }
        Ok(value)
    }

    pub(crate) fn decrement(&self, rhs: u64) -> Result<u64, SegError> {
        let value = self.value_numeric()?;
        let value = value.saturating_sub(rhs);
        unsafe {
            std::ptr::copy_nonoverlapping(
                value.to_be_bytes().as_ptr(),
                self.data.add(self.value_offset()),
                core::mem::size_of::<u64>(),
            );
        }
        Ok(value)
    }
}

impl std::fmt::Debug for RawItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        f.debug_struct("RawItem")
            .field("size", &self.size())
            .field("header", self.header())
            .field(
                "raw",
                &format!("{:02X?}", unsafe {
                    &std::slice::from_raw_parts(self.data, self.size())
                }),
            )
            .finish()
    }
}
