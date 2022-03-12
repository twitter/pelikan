// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! This module defines a `MemcacheEntry` type which is used by the protocol and
//! storage implementations to execute requests.

use std::time::Duration;
use storage_types::{OwnedValue, Value};

#[derive(Debug)]
/// Defines a `MemcacheEntry`
pub struct MemcacheEntry {
    // The key for the entry
    pub key: Box<[u8]>,
    // An optional value for the entry. A key may exist in the cache with no
    // associated value.
    pub value: Option<OwnedValue>,
    // A optional time-to-live for the entry. `None` variant indicates that the
    // item will not expire. A zero-duration should be interpreted as immediate
    // expiration.
    pub ttl: Option<Duration>,
    // Opaque flags which may be set by the client and stored alongside the
    // item.
    pub flags: u32,
    // An optional value which is used for compare-and-store (CAS) operations.
    pub cas: Option<u64>,
}

impl MemcacheEntry {
    /// Returns a reference to the key for the entry.
    pub fn key(&self) -> &[u8] {
        &self.key
    }

    /// Returns a reference to the value for the entry. The `None` variant is
    /// used when the key is present, but has no associated value.
    pub fn value(&self) -> Option<Value> {
        self.value.as_ref().map(|v| v.as_value())
    }

    /// The TTL in seconds. `None` indicates that the item will not expire.
    pub fn ttl(&self) -> Option<Duration> {
        self.ttl
    }

    /// Returns the opaque `u32` flags which are stored with alongside the item.
    pub fn flags(&self) -> u32 {
        self.flags
    }

    /// Returns the value which should be used with compare-and-store (CAS)
    /// operations.
    pub fn cas(&self) -> Option<u64> {
        self.cas
    }
}
