// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use std::sync::Arc;
use crate::protocol::CRLF_LEN;
use crate::protocol::CRLF;
use crate::protocol::data::*;
use crate::MemcacheRequest;

use bytes::BytesMut;

use std::borrow::Borrow;
use std::convert::TryFrom;

pub trait Init<Config> {
	fn new(config: Arc<Config>) -> Self;
}

pub trait Response {
	fn compose(self, buffer: &mut BytesMut);
}

pub trait Execute<Request> {
    fn execute(&mut self, request: Request) -> Box<dyn Response>;
}

pub trait Parse<Request> {
	fn parse(&mut self) -> Result<Request, ParseError>;
}

impl Parse<MemcacheRequest> for BytesMut {
	fn parse(&mut self) -> Result<MemcacheRequest, ParseError> {
        let command;
        {
            let buf: &[u8] = (*self).borrow();
            // check if we got a CRLF
            let mut double_byte = buf.windows(CRLF_LEN);
            if let Some(_line_end) = double_byte.position(|w| w == CRLF) {
                // single-byte windowing to find spaces
                let mut single_byte = buf.windows(1);
                if let Some(cmd_end) = single_byte.position(|w| w == b" ") {
                    command = MemcacheCommand::try_from(&buf[0..cmd_end])?;
                } else {
                    return Err(ParseError::Incomplete);
                }
            } else {
                return Err(ParseError::Incomplete);
            }
        }

        match command {
            MemcacheCommand::Get => MemcacheRequest::parse_get(self),
            MemcacheCommand::Gets => MemcacheRequest::parse_gets(self),
            MemcacheCommand::Set => MemcacheRequest::parse_set(self),
            MemcacheCommand::Add => MemcacheRequest::parse_add(self),
            MemcacheCommand::Replace => MemcacheRequest::parse_replace(self),
            MemcacheCommand::Cas => MemcacheRequest::parse_cas(self),
            MemcacheCommand::Delete => MemcacheRequest::parse_delete(self),
        }
    }
}

pub trait MemcacheStorage {
    fn get(&mut self, keys: &[&[u8]]) -> Box<dyn Response>;
    fn gets(&mut self, keys: &[&[u8]]) -> Box<dyn Response>;
    fn set(
        &mut self,
        key: &[u8],
        value: Option<&[u8]>,
        flags: u32,
        expiry: u32,
        noreply: bool,
    ) -> Box<dyn Response>;
    fn add(
        &mut self,
        key: &[u8],
        value: Option<&[u8]>,
        flags: u32,
        expiry: u32,
        noreply: bool,
    ) -> Box<dyn Response>;
    fn replace(
        &mut self,
        key: &[u8],
        value: Option<&[u8]>,
        flags: u32,
        expiry: u32,
        noreply: bool,
    ) -> Box<dyn Response>;
    fn delete(&mut self, key: &[u8], noreply: bool) -> Box<dyn Response>;
    fn cas(
        &mut self,
        key: &[u8],
        value: Option<&[u8]>,
        flags: u32,
        expiry: u32,
        noreply: bool,
        cas: u64,
    ) -> Box<dyn Response>;
}
