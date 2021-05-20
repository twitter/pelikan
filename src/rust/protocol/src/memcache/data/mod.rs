// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

mod request;
mod response;

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
            key: request.keys[0].clone(),
            value: request.value,
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
