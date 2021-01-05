// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

mod constants;
mod header;
mod raw;
mod reserved;

pub use constants::*;
pub use header::ItemHeader;
pub use raw::RawItem;
pub use reserved::ReservedItem;

/// An `Item` represents a stored item and is used in the public interface of
/// `SegCache`.
pub struct Item {
    cas: u32,
    raw: RawItem,
}

impl Item {
    /// Creates a new `Item` from its parts
    pub(crate) fn new(raw: RawItem, cas: u32) -> Self {
        Item { raw, cas }
    }

    /// Check the item's magic
    pub fn check_magic(&self) {
        self.raw.check_magic()
    }

    /// Borrow the item key
    pub fn key(&self) -> &[u8] {
        self.raw.key()
    }

    /// Borrow the item value
    pub fn value(&self) -> &[u8] {
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
