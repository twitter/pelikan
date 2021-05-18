// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

pub struct MemcacheItem {
    pub key: Box<[u8]>,
    pub value: Box<[u8]>,
    pub flags: u32,
    pub cas: Option<u32>,
}
