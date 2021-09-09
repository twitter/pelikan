// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

#[macro_use]
extern crate logger;

use session::Session;

pub mod admin;
pub mod memcache;
pub mod ping;

pub const CRLF: &str = "\r\n";

pub trait Compose {
    fn compose(self, dst: &mut Session);
}

pub trait Execute<Request, Response> {
    fn execute(&mut self, request: Request) -> Option<Response>;
}

#[derive(Debug, PartialEq)]
pub enum ParseError {
    Invalid,
    Incomplete,
    UnknownCommand,
}

#[derive(Debug, PartialEq)]
pub struct ParseOk<T> {
    message: T,
    consumed: usize,
}

impl<T> ParseOk<T> {
    pub fn into_inner(self) -> T {
        self.message
    }

    pub fn consumed(&self) -> usize {
        self.consumed
    }
}

pub trait Parse<T> {
    fn parse(&self, buffer: &[u8]) -> Result<ParseOk<T>, ParseError>;
}

metrics::test_no_duplicates!();
