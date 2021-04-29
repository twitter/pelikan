// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::item::*;

// NOTE: repr(packed) is necessary to get the smallest representation. The
// struct is always taken from an aligned pointer cast. This can potentially
// result in UB when fields are referenced. Fields that require access by
// reference must be strategically placed to ensure alignment and avoid UB.

#[repr(C)]
#[repr(packed)]
pub struct ItemHeader {
    #[cfg(feature = "magic")]
    magic: u32,
    len: u32,  // packs vlen:24 klen:8
    flags: u8, // packs is_num:1, deleted:1, olen:6
}

#[cfg(not(feature = "magic"))]
impl std::fmt::Debug for ItemHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        f.debug_struct("ItemHeader")
            .field("klen", &self.klen())
            .field("vlen", &self.vlen())
            .field("is_num", &self.is_num())
            .field("deleted", &self.is_deleted())
            .field("olen", &self.olen())
            .finish()
    }
}

#[cfg(feature = "magic")]
impl std::fmt::Debug for ItemHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        let magic = self.magic;
        f.debug_struct("ItemHeader")
            .field("magic", &format!("0x{:X}", magic))
            .field("klen", &self.klen())
            .field("vlen", &self.vlen())
            .field("is_num", &self.is_num())
            .field("deleted", &self.is_deleted())
            .field("olen", &self.olen())
            .finish()
    }
}

impl ItemHeader {
    /// Get the magic bytes from the header
    #[cfg(feature = "magic")]
    #[inline]
    pub fn magic(&self) -> u32 {
        self.magic
    }

    /// Write the magic bytes into the header
    #[cfg(feature = "magic")]
    #[inline]
    pub fn set_magic(&mut self) {
        self.magic = ITEM_MAGIC;
    }

    /// Check the magic bytes
    #[inline]
    pub fn check_magic(&self) {
        #[cfg(feature = "magic")]
        assert_eq!(self.magic(), ITEM_MAGIC);
    }

    /// Get the item's key length
    #[inline]
    pub fn klen(&self) -> u8 {
        self.len as u8
    }

    /// Get the item's value length
    #[inline]
    pub fn vlen(&self) -> u32 {
        self.len >> VLEN_SHIFT
    }

    /// get the optional data length
    #[inline]
    pub fn olen(&self) -> u8 {
        self.flags & OLEN_MASK
    }

    /// Is the item a numeric value?
    #[inline]
    pub fn is_num(&self) -> bool {
        self.flags & NUM_MASK != 0
    }

    /// Is the item deleted?
    #[inline]
    pub fn is_deleted(&self) -> bool {
        self.flags & DEL_MASK != 0
    }

    /// Set the key length by changing just the low byte
    #[inline]
    pub fn set_klen(&mut self, len: u8) {
        self.len = (self.len & !KLEN_MASK) | (len as u32);
    }

    /// Set the value length by changing just the upper bytes
    // TODO(bmartin): where should we do error handling for out-of-range?
    #[inline]
    pub fn set_vlen(&mut self, len: u32) {
        debug_assert!(len <= (u32::MAX >> VLEN_SHIFT));
        self.len = (self.len & !VLEN_MASK) | (len << VLEN_SHIFT);
    }

    /// Mark the item as deleted
    #[inline]
    pub fn set_deleted(&mut self, deleted: bool) {
        if deleted {
            self.flags |= DEL_MASK
        } else {
            self.flags &= !DEL_MASK
        }
    }

    /// Mark the item as numeric
    #[inline]
    pub fn set_num(&mut self, num: bool) {
        if num {
            self.flags |= NUM_MASK
        } else {
            self.flags &= !NUM_MASK
        }
    }

    /// Set the optional length
    #[inline]
    pub fn set_olen(&mut self, len: u8) {
        debug_assert!(len <= OLEN_MASK);
        self.flags = (self.flags & !OLEN_MASK) | len;
    }
}