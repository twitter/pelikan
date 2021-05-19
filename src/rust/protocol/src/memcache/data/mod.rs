// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

mod item;
mod request;
mod response;

pub use item::*;
pub use request::*;
pub use response::*;

use super::*;
use crate::*;

impl<T> Execute<MemcacheRequest, MemcacheResponse> for T
where
    T: MemcacheStorage,
{
    fn execute(&mut self, request: MemcacheRequest) -> MemcacheResponse {
        match request.command {
            MemcacheCommand::Get => self.get(&request.keys),
            MemcacheCommand::Gets => self.gets(&request.keys),
            MemcacheCommand::Set => self.set(
                &request.keys[0],
                request.value,
                request.flags,
                request.expiry,
                request.noreply,
            ),
            MemcacheCommand::Add => self.add(
                &request.keys[0],
                request.value,
                request.flags,
                request.expiry,
                request.noreply,
            ),
            MemcacheCommand::Replace => self.replace(
                &request.keys[0],
                request.value,
                request.flags,
                request.expiry,
                request.noreply,
            ),
            MemcacheCommand::Delete => self.delete(&request.keys[0], request.noreply),
            MemcacheCommand::Cas => self.cas(
                &request.keys[0],
                request.value,
                request.flags,
                request.expiry,
                request.noreply,
                request.cas,
            ),
        }
    }
}
