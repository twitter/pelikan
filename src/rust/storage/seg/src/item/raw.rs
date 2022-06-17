// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! A raw byte-level representation of an item.
//!
//! Unlike an [`Item`], the [`RawItem`] does not contain any fields which are
//! shared within a hash bucket such as the CAS value.

use super::header::ValueType;
use crate::item::*;
use crate::SegError;
use crate::Value;

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

    /// Returns the value length as stored in the header
    #[inline]
    fn vlen(&self) -> u32 {
        self.header().vlen()
    }

    /// Borrow the value
    // TODO(bmartin): should probably change this to be Option<>
    pub(crate) fn value(&self) -> Value {
        let bytes = unsafe {
            let ptr = self.data.add(self.value_offset());
            let len = self.vlen() as usize;
            std::slice::from_raw_parts(ptr, len)
        };

        // TODO(bmartin): consider allowing other return types here by encoding
        // the type in the vlen portion of the header
        match self.header().value_type() {
            Some(ValueType::U64) => Value::U64(u64::from_be_bytes([
                bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
            ])),
            None => Value::Bytes(bytes),
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
    pub(crate) fn define(&mut self, key: &[u8], value: Value, optional: &[u8]) {
        unsafe { (*self.header_mut()).init(); }
        match value {
            Value::Bytes(value) => unsafe {
                self.set_magic();
                (*self.header_mut()).set_type(None);
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
                (*self.header_mut()).set_vlen(value.len() as u32);
                std::ptr::copy_nonoverlapping(
                    value.as_ptr(),
                    self.data.add(self.value_offset()),
                    value.len(),
                );
            },
            Value::U64(value) => unsafe {
                self.set_magic();
                (*self.header_mut()).set_type(Some(ValueType::U64));
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
                let bytes = value.to_be_bytes();
                std::ptr::copy_nonoverlapping(
                    bytes.as_ptr(),
                    self.data.add(self.value_offset()),
                    bytes.len(),
                );
            },
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

    pub(crate) fn wrapping_add(&mut self, rhs: u64) -> Result<(), SegError> {
        match self.value() {
            Value::U64(v) => unsafe {
                let new = v.wrapping_add(rhs);
                std::ptr::copy_nonoverlapping(
                    new.to_be_bytes().as_ptr(),
                    self.data.add(self.value_offset()),
                    core::mem::size_of::<u64>(),
                );
                Ok(())
            },
            _ => Err(SegError::NotNumeric),
        }
    }

    pub(crate) fn saturating_sub(&mut self, rhs: u64) -> Result<(), SegError> {
        match self.value() {
            Value::U64(v) => unsafe {
                let new = v.saturating_sub(rhs);
                std::ptr::copy_nonoverlapping(
                    new.to_be_bytes().as_ptr(),
                    self.data.add(self.value_offset()),
                    core::mem::size_of::<u64>(),
                );
                Ok(())
            },
            _ => Err(SegError::NotNumeric),
        }
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
