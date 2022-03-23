// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! This module defines all possible `Memcache` commands.

use crate::ParseError;
use core::convert::TryFrom;

/// Memcache protocol commands
#[derive(PartialEq)]
pub enum MemcacheCommand {
    Get,
    Gets,
    Set,
    Add,
    Replace,
    Append,
    Prepend,
    Delete,
    Incr,
    Decr,
    Cas,
    Quit,
    FlushAll,
}

impl TryFrom<&[u8]> for MemcacheCommand {
    type Error = ParseError;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        let cmd = match value {
            b"get" => MemcacheCommand::Get,
            b"gets" => MemcacheCommand::Gets,
            b"set" => MemcacheCommand::Set,
            b"add" => MemcacheCommand::Add,
            b"replace" => MemcacheCommand::Replace,
            b"append" => MemcacheCommand::Append,
            b"prepend" => MemcacheCommand::Prepend,
            b"cas" => MemcacheCommand::Cas,
            b"delete" => MemcacheCommand::Delete,
            b"incr" => MemcacheCommand::Incr,
            b"decr" => MemcacheCommand::Decr,
            b"quit" => MemcacheCommand::Quit,
            b"flush_all" => MemcacheCommand::FlushAll,
            _ => {
                return Err(ParseError::Unknown);
            }
        };
        Ok(cmd)
    }
}

impl std::fmt::Display for MemcacheCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        let name = match self {
            Self::Get => "get",
            Self::Gets => "gets",
            Self::Set => "set",
            Self::Add => "add",
            Self::Replace => "replace",
            Self::Append => "append",
            Self::Prepend => "prepend",
            Self::Cas => "cas",
            Self::Delete => "delete",
            Self::Incr => "incr",
            Self::Decr => "decr",
            Self::Quit => "quit",
            Self::FlushAll => "flush_all",
        };
        write!(f, "{}", name)
    }
}
