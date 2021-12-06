// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

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
    Delete { key: Key, noreply: bool },
    Cas { entry: MemcacheEntry, noreply: bool },
    FlushAll,
}

impl MemcacheRequest {
    pub fn noreply(&self) -> bool {
        match self {
            Self::Set { noreply, .. } => *noreply,
            Self::Add { noreply, .. } => *noreply,
            Self::Replace { noreply, .. } => *noreply,
            Self::Delete { noreply, .. } => *noreply,
            Self::Cas { noreply, .. } => *noreply,
            _ => false,
        }
    }

    pub fn key(&self) -> Result<&[u8], ()> {
        match self {
            Self::Set { entry, .. } => Ok(entry.key()),
            Self::Add { entry, .. } => Ok(entry.key()),
            Self::Replace { entry, .. } => Ok(entry.key()),
            Self::Delete { key, .. } => Ok(key.as_ref()),
            Self::Cas { entry, .. } => Ok(entry.key()),
            _ => Err(()),
        }
    }

    pub fn entry(&self) -> Option<&MemcacheEntry> {
        match self {
            Self::Set { entry, .. } => Some(entry),
            Self::Add { entry, .. } => Some(entry),
            Self::Replace { entry, .. } => Some(entry),
            Self::Cas { entry, .. } => Some(entry),
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
            Self::Delete { .. } => MemcacheCommand::Delete,
            Self::Cas { .. } => MemcacheCommand::Cas,
            Self::FlushAll => MemcacheCommand::FlushAll,
        }
    }
}
