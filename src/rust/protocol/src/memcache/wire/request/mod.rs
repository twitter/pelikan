// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Implements all request parsing and validation for the `Memcache` protocol.

mod command;
mod parse;

#[cfg(test)]
mod test;

use crate::memcache::MemcacheEntry;
pub use command::MemcacheCommand;

pub use parse::{MemcacheRequestParser, MAX_BATCH_SIZE};

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
    Append { entry: MemcacheEntry, noreply: bool },
    Prepend { entry: MemcacheEntry, noreply: bool },
    Delete { key: Key, noreply: bool },
    Incr { key: Key, value: u64, noreply: bool },
    Decr { key: Key, value: u64, noreply: bool },
    Cas { entry: MemcacheEntry, noreply: bool },
    FlushAll,
    Stop,
}

impl MemcacheRequest {
    pub fn noreply(&self) -> bool {
        match self {
            Self::Set { noreply, .. } => *noreply,
            Self::Add { noreply, .. } => *noreply,
            Self::Replace { noreply, .. } => *noreply,
            Self::Append { noreply, .. } => *noreply,
            Self::Prepend { noreply, .. } => *noreply,
            Self::Delete { noreply, .. } => *noreply,
            Self::Incr { noreply, .. } => *noreply,
            Self::Decr { noreply, .. } => *noreply,
            Self::Cas { noreply, .. } => *noreply,
            _ => false,
        }
    }

    pub fn key(&self) -> Result<&[u8], ()> {
        match self {
            Self::Set { entry, .. } => Ok(entry.key()),
            Self::Add { entry, .. } => Ok(entry.key()),
            Self::Replace { entry, .. } => Ok(entry.key()),
            Self::Append { entry, .. } => Ok(entry.key()),
            Self::Prepend { entry, .. } => Ok(entry.key()),
            Self::Delete { key, .. } => Ok(key.as_ref()),
            Self::Incr { key, .. } => Ok(key.as_ref()),
            Self::Decr { key, .. } => Ok(key.as_ref()),
            Self::Cas { entry, .. } => Ok(entry.key()),
            _ => Err(()),
        }
    }

    pub fn entry(&self) -> Option<&MemcacheEntry> {
        match self {
            Self::Set { entry, .. } => Some(entry),
            Self::Add { entry, .. } => Some(entry),
            Self::Replace { entry, .. } => Some(entry),
            Self::Append { entry, .. } => Some(entry),
            Self::Prepend { entry, .. } => Some(entry),
            Self::Cas { entry, .. } => Some(entry),
            _ => None,
        }
    }

    pub fn count(&self) -> Option<u64> {
        match self {
            Self::Incr { value, .. } => Some(*value),
            Self::Decr { value, .. } => Some(*value),
            _ => None,
        }
    }

    pub fn command(&self) -> MemcacheCommand {
        match self {
            Self::Get { .. } => MemcacheCommand::Get,
            Self::Gets { .. } => MemcacheCommand::Gets,
            Self::Set { .. } => MemcacheCommand::Set,
            Self::Add { .. } => MemcacheCommand::Add,
            Self::Replace { .. } => MemcacheCommand::Replace,
            Self::Append { .. } => MemcacheCommand::Append,
            Self::Prepend { .. } => MemcacheCommand::Prepend,
            Self::Delete { .. } => MemcacheCommand::Delete,
            Self::Incr { .. } => MemcacheCommand::Incr,
            Self::Decr { .. } => MemcacheCommand::Decr,
            Self::Cas { .. } => MemcacheCommand::Cas,
            Self::FlushAll => MemcacheCommand::FlushAll,
            Self::Stop => MemcacheCommand::Stop,

        }
    }
}
