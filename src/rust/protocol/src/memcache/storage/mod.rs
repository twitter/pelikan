// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::memcache::MemcacheEntry;
use super::data::MemcacheResponse;
// use common::expiry::Expiry;


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
