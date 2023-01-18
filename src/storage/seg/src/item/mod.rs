// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Items are the base unit of data stored within the cache.

mod header;
mod raw;
mod reserved;

#[cfg(any(feature = "magic", feature = "debug"))]
pub(crate) use header::ITEM_MAGIC_SIZE;

use crate::hashtable::FREQ_MASK;
use crate::SegError;
use crate::Value;

pub(crate) use header::{ItemHeader, ITEM_HDR_SIZE};
pub(crate) use raw::RawItem;
pub(crate) use reserved::ReservedItem;

/// Items are the base unit of data stored within the cache.
pub struct Item {
    cas: u32,
    age: u32,
    raw: RawItem,
}

impl Item {
    /// Creates a new `Item` from its parts
    pub(crate) fn new(raw: RawItem, age: u32, cas: u32) -> Self {
        Item { cas, age, raw }
    }

    /// If the `magic` or `debug` features are enabled, this allows for checking
    /// that the magic bytes at the start of an item match the expected value.
    ///
    /// # Panics
    ///
    /// Panics if the magic bytes are incorrect, indicating that the data has
    /// become corrupted or the item was loaded from the wrong offset.
    pub(crate) fn check_magic(&self) {
        self.raw.check_magic()
    }

    /// Borrow the item key
    pub fn key(&self) -> &[u8] {
        self.raw.key()
    }

    /// Borrow the item value
    pub fn value(&self) -> Value {
        self.raw.value()
    }

    /// CAS value for the item
    pub fn cas(&self) -> u32 {
        self.cas
    }

    pub fn age(&self) -> u32 {
        self.age
    }

    /// Borrow the optional data
    pub fn optional(&self) -> Option<&[u8]> {
        self.raw.optional()
    }

    /// Perform a wrapping addition on the value. Returns an error if the item
    /// is not a numeric type.
    pub fn wrapping_add(&mut self, rhs: u64) -> Result<(), SegError> {
        self.raw.wrapping_add(rhs)
    }

    /// Perform a saturating subtraction on the value. Returns an error if the
    /// item is not a numeric type.
    pub fn saturating_sub(&mut self, rhs: u64) -> Result<(), SegError> {
        self.raw.saturating_sub(rhs)
    }
}

impl std::fmt::Debug for Item {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        f.debug_struct("Item")
            .field("cas", &self.cas())
            .field("raw", &self.raw)
            .finish()
    }
}

/// Items are the base unit of data stored within the cache.
pub struct RichItem {
    item: Item,
    item_info: u64,
    item_info_ptr: *const u64,
}

impl RichItem {
    /// Creates a new `Item` from its parts
    pub(crate) fn new(
        raw: RawItem,
        age: u32,
        cas: u32,
        item_info: u64,
        item_info_ptr: *const u64,
    ) -> Self {
        let item = Item::new(raw, age, cas);
        RichItem {
            item,
            item_info,
            item_info_ptr,
        }
    }

    /// If the `magic` or `debug` features are enabled, this allows for checking
    /// that the magic bytes at the start of an item match the expected value.
    ///
    /// # Panics
    ///
    /// Panics if the magic bytes are incorrect, indicating that the data has
    /// become corrupted or the item was loaded from the wrong offset.
    pub(crate) fn check_magic(&self) {
        self.item.raw.check_magic()
    }

    /// Borrow the item key
    pub fn key(&self) -> &[u8] {
        self.item.raw.key()
    }

    /// Borrow the item value
    pub fn value(&self) -> Value {
        self.item.raw.value()
    }

    /// CAS value for the item
    pub fn cas(&self) -> u32 {
        self.item.cas
    }

    pub fn age(&self) -> u32 {
        self.item.age
    }

    pub fn item(&self) -> &Item {
        &self.item
    }

    pub fn item_mut(&mut self) -> &mut Item {
        &mut self.item
    }

    // used to support multi readers and single writer
    // return true, if the item is evicted/updated since being
    // read from the hash table
    pub fn is_not_changed(&self) -> bool {
        unsafe { return self.item_info == *self.item_info_ptr & !FREQ_MASK }
    }

    /// Borrow the optional data
    pub fn optional(&self) -> Option<&[u8]> {
        self.item.raw.optional()
    }

    /// Perform a wrapping addition on the value. Returns an error if the item
    /// is not a numeric type.
    pub fn wrapping_add(&mut self, rhs: u64) -> Result<(), SegError> {
        self.item.raw.wrapping_add(rhs)
    }

    /// Perform a saturating subtraction on the value. Returns an error if the
    /// item is not a numeric type.
    pub fn saturating_sub(&mut self, rhs: u64) -> Result<(), SegError> {
        self.item.raw.saturating_sub(rhs)
    }
}

impl std::fmt::Debug for RichItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        f.debug_struct("Item")
            .field("cas", &self.cas())
            .field("raw", &self.item.raw)
            .field("item_info", &self.item_info)
            .field("item_info_ptr", &self.item_info_ptr)
            .finish()
    }
}

pub fn size_of(value: &Value) -> usize {
    match value {
        Value::Bytes(v) => v.len(),
        Value::U64(_) => core::mem::size_of::<u64>(),
    }
}
