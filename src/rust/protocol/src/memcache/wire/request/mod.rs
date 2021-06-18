// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

mod command;
mod parse;

#[cfg(test)]
mod test;

use crate::memcache::MemcacheEntry;
pub use command::MemcacheCommand;

pub use parse::MemcacheRequestParser;

pub const NOREPLY: &str = "noreply";

pub type Key = Box<[u8]>;
pub type Keys = Box<[Key]>;

#[derive(Debug)]
pub enum MemcacheRequest {
    Get { keys: Keys },
    Gets { keys: Keys },
    Set { entry: MemcacheEntry, noreply: bool },
    Add { entry: MemcacheEntry, noreply: bool },
    Replace { entry: MemcacheEntry, noreply: bool },
    Delete { key: Key, noreply: bool },
    Cas { entry: MemcacheEntry, noreply: bool },
}
