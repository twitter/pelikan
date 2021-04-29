// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::RawItem;

/// `ReservedItem` represents an item which has been allocated but is not
/// defined or linked in the hashtable yet.
#[derive(Debug)]
pub struct ReservedItem {
    item: RawItem,
    seg: i32,
    offset: usize,
}

impl ReservedItem {
    /// Create a `ReservedItem` from its parts
    pub fn new(item: RawItem, seg: i32, offset: usize) -> Self {
        Self { item, seg, offset }
    }

    #[cfg(feature = "magic")]
    /// Check the item magic
    pub fn check_magic(&self) {
        self.item.check_magic()
    }

    /// Store the key, value, and optional data into the item
    pub fn define(&mut self, key: &[u8], value: &[u8], optional: &[u8]) {
        self.item.define(key, value, optional)
    }

    /// Get the `RawItem` that backs the `ReservedItem`
    pub fn item(&self) -> RawItem {
        self.item
    }

    /// Get the segment offset
    pub fn offset(&self) -> usize {
        self.offset
    }

    /// Get the segment id
    pub fn seg(&self) -> i32 {
        self.seg
    }
}
