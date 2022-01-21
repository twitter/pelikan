// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use std::time::Duration;

#[derive(Debug)]
pub struct MemcacheEntry {
    pub key: Box<[u8]>,
    pub value: Option<Box<[u8]>>,
    pub ttl: Option<Duration>,
    pub flags: u32,
    pub cas: Option<u64>,
}

impl MemcacheEntry {
    pub fn key(&self) -> &[u8] {
        &self.key
    }

    pub fn value(&self) -> Option<&[u8]> {
        if self.value.is_some() {
            Some(self.value.as_ref().unwrap().as_ref())
        } else {
            None
        }
    }

    /// The TTL in seconds. `None` indicates that the item will not expire.
    pub fn ttl(&self) -> Option<Duration> {
        self.ttl
    }

    pub fn flags(&self) -> u32 {
        self.flags
    }

    pub fn cas(&self) -> Option<u64> {
        self.cas
    }
}
