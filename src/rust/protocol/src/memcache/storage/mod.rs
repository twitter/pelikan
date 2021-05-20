// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::memcache::MemcacheEntry;

pub enum MemcacheStorageError {
    Exists,
    NotFound,
    NotStored,
}

/// Defines operations that arbitrary storage must be able to handle to be used
/// as storage in a Memcache-like backend.
pub trait MemcacheStorage {
    /// Lookup the specified key(s) and return entries that exist
    fn get(&mut self, keys: &[Box<[u8]>]) -> Box<[MemcacheEntry]>;

    /// Store an entry and return a response
    fn set(&mut self, entry: MemcacheEntry) -> Result<(), MemcacheStorageError>;

    /// Stores an item if the key is not currently in the cache
    fn add(&mut self, entry: MemcacheEntry) -> Result<(), MemcacheStorageError>;

    /// Stores an item only if the key is already in the cache
    fn replace(&mut self, entry: MemcacheEntry) -> Result<(), MemcacheStorageError>;

    /// Remove the item with the specified key
    fn delete(&mut self, key: &[u8]) -> Result<(), MemcacheStorageError>;

    /// Compare and store on the CAS value, replacing the stored item if the CAS
    /// value matches the provided value.
    fn cas(&mut self, entry: MemcacheEntry) -> Result<(), MemcacheStorageError>;
}
