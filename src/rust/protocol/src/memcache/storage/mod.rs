// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use super::data::MemcacheResponse;

pub trait MemcacheStorage {
    /// Lookup the specified key(s) and return the corresponding items
    fn get(&mut self, keys: &[Box<[u8]>]) -> MemcacheResponse;

    /// Lookup the specified key(s) and return their CAS values and
    /// corresponding items
    fn gets(&mut self, keys: &[Box<[u8]>]) -> MemcacheResponse;

    /// Store an item and return a response
    fn set(
        &mut self,
        key: &[u8],
        value: Option<Box<[u8]>>,
        flags: u32,
        expiry: u32,
        noreply: bool,
    ) -> MemcacheResponse;

    /// Stores an item if the key is not currently in the cache
    fn add(
        &mut self,
        key: &[u8],
        value: Option<Box<[u8]>>,
        flags: u32,
        expiry: u32,
        noreply: bool,
    ) -> MemcacheResponse;

    /// Stores an item only if the key is already in the cache
    fn replace(
        &mut self,
        key: &[u8],
        value: Option<Box<[u8]>>,
        flags: u32,
        expiry: u32,
        noreply: bool,
    ) -> MemcacheResponse;

    /// Remove the item with the specified key
    fn delete(&mut self, key: &[u8], noreply: bool) -> MemcacheResponse;

    /// Compare and store on the CAS value, replacing the stored item if the CAS
    /// value matches the provided value.
    fn cas(
        &mut self,
        key: &[u8],
        value: Option<Box<[u8]>>,
        flags: u32,
        expiry: u32,
        noreply: bool,
        cas: u64,
    ) -> MemcacheResponse;
}
