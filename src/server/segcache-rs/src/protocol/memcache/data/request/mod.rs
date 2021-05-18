// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

mod command;
mod parse;

pub use command::MemcacheCommand;

pub const NOREPLY: &str = "noreply";

pub struct MemcacheRequest {
    pub(crate) command: MemcacheCommand,
    pub(crate) keys: Box<[Box<[u8]>]>,
    pub(crate) value: Option<Box<[u8]>>,
    pub(crate) flags: u32,
    pub(crate) expiry: u32,
    pub(crate) noreply: bool,
    pub(crate) cas: u64,
}
