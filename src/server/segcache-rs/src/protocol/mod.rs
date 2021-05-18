// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

pub mod admin;
pub mod memcache;

use crate::buffer::Buffer;

pub const CRLF: &str = "\r\n";

pub trait Compose {
    fn compose(self, buffer: &mut Buffer);
}

pub trait Execute<Request, Response> {
    fn execute(&mut self, request: Request) -> Response;
}

pub enum ParseError {
    Invalid,
    Incomplete,
    UnknownCommand,
}

pub trait Parse<Buffer>
where
    Self: Sized,
{
    fn parse(buffer: &mut Buffer) -> Result<Self, ParseError>;
}
