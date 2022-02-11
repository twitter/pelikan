// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! This module defines what operations that a storage implementation must
//! implement to be used as storage for the `Memcache` protocol.

use crate::memcache::MemcacheEntry;

pub enum MemcacheStorageError {
    Exists,
    NotFound,
    NotStored,
    NotSupported,
    NotNumeric,
}

/// Defines operations that arbitrary storage must be able to handle to be used
/// as storage in a Memcache-like backend.
pub trait MemcacheStorage {
    /// Lookup the specified key(s) and return entries that exist
    fn get(&mut self, keys: &[Box<[u8]>]) -> Box<[MemcacheEntry]>;

    /// Store an entry and return a response
    fn set(&mut self, entry: &MemcacheEntry) -> Result<(), MemcacheStorageError>;

    /// Stores an item if the key is not currently in the cache
    fn add(&mut self, entry: &MemcacheEntry) -> Result<(), MemcacheStorageError>;

    /// Stores an item only if the key is already in the cache
    fn replace(&mut self, entry: &MemcacheEntry) -> Result<(), MemcacheStorageError>;

    /// Appends the value in the entry to the existing value if the key is
    /// already present in the cache. Ignores expiry and flags.
    fn append(&mut self, entry: &MemcacheEntry) -> Result<(), MemcacheStorageError>;

    /// Prepends the value in the entry to the existing value if the key is
    /// already present in the cache. Ignores expiry and flags.
    fn prepend(&mut self, entry: &MemcacheEntry) -> Result<(), MemcacheStorageError>;

    /// Increment the value for the key by the provided value if it is already
    /// present in the cache. Addition is treated as overflowing arithmetic.
    fn incr(&mut self, key: &[u8], value: u64) -> Result<u64, MemcacheStorageError>;

    /// Decrement the value for the key by the provided value if it is already
    /// present in the cache. Subtraction is treated as saturating arithmetic.
    fn decr(&mut self, key: &[u8], value: u64) -> Result<u64, MemcacheStorageError>;

    /// Remove the item with the specified key
    fn delete(&mut self, key: &[u8]) -> Result<(), MemcacheStorageError>;

    /// Compare and store on the CAS value, replacing the stored item if the CAS
    /// value matches the provided value.
    fn cas(&mut self, entry: &MemcacheEntry) -> Result<(), MemcacheStorageError>;
}
