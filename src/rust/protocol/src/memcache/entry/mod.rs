// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

#[derive(Debug)]
pub struct MemcacheEntry {
    pub key: Box<[u8]>,
    pub value: Box<[u8]>,
    pub ttl: Option<u32>,
    pub flags: u32,
    pub cas: Option<u64>,
}

impl MemcacheEntry {
    pub fn key(&self) -> &[u8] {
        &self.key
    }

    pub fn value(&self) -> &[u8] {
        &self.value
    }

    /// The TTL in seconds. `Some(0)` indicates immediate expiration. `None`
    /// indicates that the item will not expire.
    pub fn ttl(&self) -> Option<u32> {
        self.ttl
    }

    pub fn flags(&self) -> u32 {
        self.flags
    }

    pub fn cas(&self) -> Option<u64> {
        self.cas
    }
}
