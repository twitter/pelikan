// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Items are the base unit of data stored within the cache.

mod header;
mod raw;
mod reserved;

#[cfg(any(feature = "magic", feature = "debug"))]
pub(crate) use header::ITEM_MAGIC_SIZE;

use crate::Value;

pub(crate) use header::{ItemHeader, ITEM_HDR_SIZE};
pub(crate) use raw::RawItem;
pub(crate) use reserved::ReservedItem;

/// Items are the base unit of data stored within the cache.
pub struct Item {
    cas: u32,
    raw: RawItem,
}

impl Item {
    /// Creates a new `Item` from its parts
    pub(crate) fn new(raw: RawItem, cas: u32) -> Self {
        Item { cas, raw }
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

    /// Borrow the optional data
    pub fn optional(&self) -> Option<&[u8]> {
        self.raw.optional()
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

pub fn size_of(value: &Value) -> usize {
    match value {
        Value::Bytes(v) => v.len(),
        Value::U64(_) => core::mem::size_of::<u64>(),
        Value::I64(_) => core::mem::size_of::<i64>(),
    }
}
