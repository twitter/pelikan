// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use super::data::MemcacheResponse;
// use common::expiry::Expiry;

pub struct MemcacheEntry<'a> {
    pub(crate) key: &'a [u8],
    pub(crate) value: &'a [u8],
    pub(crate) expiry: u32,
    pub(crate) flags: u32,
    pub(crate) cas: u64,
}

impl<'a> MemcacheEntry<'a> {
    pub fn key(&self) -> &[u8] {
        self.key
    }

    pub fn value(&self) -> &[u8] {
        self.value
    }

    pub fn expiry(&self) -> u32 {
        self.expiry
    }

    pub fn flags(&self) -> u32 {
        self.flags
    }

    pub fn cas(&self) -> u64 {
        self.cas
    }
}

/// Defines operations that arbitrary storage must be able to handle to be used
/// as storage in a Memcache-like backend.
pub trait MemcacheStorage {
    /// Lookup the specified key(s) and return the corresponding items
    fn get(&mut self, keys: &[Box<[u8]>]) -> MemcacheResponse;

    /// Lookup the specified key(s) and return their CAS values and
    /// corresponding items
    fn gets(&mut self, keys: &[Box<[u8]>]) -> MemcacheResponse;

    /// Store an item and return a response
    fn set(
        &mut self,
        entry: MemcacheEntry
    ) -> MemcacheResponse;

    /// Stores an item if the key is not currently in the cache
    fn add(
        &mut self,
        entry: MemcacheEntry
    ) -> MemcacheResponse;

    /// Stores an item only if the key is already in the cache
    fn replace(
        &mut self,
        entry: MemcacheEntry
    ) -> MemcacheResponse;

    /// Remove the item with the specified key
    fn delete(&mut self, key: &[u8]) -> MemcacheResponse;

    /// Compare and store on the CAS value, replacing the stored item if the CAS
    /// value matches the provided value.
    fn cas(
        &mut self,
        entry: MemcacheEntry
    ) -> MemcacheResponse;
}
