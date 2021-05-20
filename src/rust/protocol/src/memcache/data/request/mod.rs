// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

mod command;
mod parse;

pub use command::MemcacheCommand;

pub const NOREPLY: &str = "noreply";

pub struct MemcacheRequest {
    /// The command type
    pub(crate) command: MemcacheCommand,
    /// The key(s) for the command
    pub(crate) keys: Box<[Box<[u8]>]>,
    /// Optional value for the request
    pub(crate) value: Box<[u8]>,
    /// Item flags
    pub(crate) flags: u32,
    /// Item expiration. *NOTE* this is not strictly a TTL
    pub(crate) expiry: u32,
    /// Server should not produce response for the request
    pub(crate) noreply: bool,
    /// CAS value. Defaults to `0`.
    pub(crate) cas: Option<u64>,
}
