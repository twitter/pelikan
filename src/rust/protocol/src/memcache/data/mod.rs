// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

mod item;
mod request;
mod response;

use crate::memcache::storage::MemcacheEntry;
pub use item::*;
pub use request::*;
pub use response::*;

use super::*;
use crate::*;

impl<'a, T> Execute<MemcacheRequest, MemcacheResponse> for T
where
    T: MemcacheStorage,
{
    fn execute(&mut self, request: MemcacheRequest) -> MemcacheResponse {
        let entry = MemcacheEntry {
            key: &request.keys[0],
            value: request.value.as_ref(),
            flags: request.flags,
            expiry: request.expiry,
            cas: request.cas,
        };

        match request.command {
            MemcacheCommand::Get => self.get(&request.keys),
            MemcacheCommand::Gets => self.gets(&request.keys),
            MemcacheCommand::Set => self.set(entry),
            MemcacheCommand::Add => self.add(entry),
            MemcacheCommand::Replace => self.replace(entry),
            MemcacheCommand::Delete => self.delete(&request.keys[0]),
            MemcacheCommand::Cas => self.cas(entry),
        }
    }
}
