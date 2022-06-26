// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::*;
use protocol_common::Parse;
use protocol_common::{ParseError, ParseOk};
use session::Session;

mod get;
mod set;

pub use get::Get;
pub use set::Set;

pub const DEFAULT_MAX_KEY_LEN: usize = 250;
pub const DEFAULT_MAX_VALUE_SIZE: usize = 512 * 1024 * 1024; // 512MB max value size

#[derive(Copy, Clone)]
pub struct RequestParser {
    max_key_len: usize,
    max_value_size: usize,
}

impl Default for RequestParser {
    fn default() -> Self {
        Self {
            max_value_size: DEFAULT_MAX_VALUE_SIZE,
            max_key_len: DEFAULT_MAX_KEY_LEN,
        }
    }
}

impl RequestParser {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn max_value_size(mut self, bytes: usize) -> Self {
        self.max_value_size = bytes;
        self
    }

    pub fn max_key_len(mut self, bytes: usize) -> Self {
        self.max_key_len = bytes;
        self
    }

    pub(crate) fn parse_request<'a>(&self, input: &'a [u8]) -> IResult<&'a [u8], Request> {
        match command(input)? {
            (input, Command::Get) => {
                let (input, request) = self.parse_get(input)?;
                Ok((input, Request::Get(request)))
            }
            (input, Command::Set) => {
                let (input, request) = self.parse_set(input)?;
                Ok((input, Request::Set(request)))
            }
        }
    }
}

impl Parse<Request> for RequestParser {
    fn parse(&self, buffer: &[u8]) -> Result<ParseOk<Request>, protocol_common::ParseError> {
        match self.parse_request(buffer) {
            Ok((input, request)) => Ok(ParseOk::new(request, buffer.len() - input.len())),
            Err(Err::Incomplete(_)) => Err(ParseError::Incomplete),
            Err(_) => Err(ParseError::Invalid),
        }
    }
}

impl Compose for Request {
    fn compose(&self, session: &mut Session) {
        match self {
            Self::Get(r) => r.compose(session),
            Self::Set(r) => r.compose(session),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Request {
    Get(Get),
    Set(Set),
}

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Get,
    Set,
}

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum ExpireTime {
    Seconds(u64),
    Milliseconds(u64),
    UnixSeconds(u64),
    UnixMilliseconds(u64),
    KeepTtl,
}

pub(crate) fn command_bytes(input: &[u8]) -> IResult<&[u8], &[u8]> {
    alphanumeric1(input)
}

pub(crate) fn command(input: &[u8]) -> IResult<&[u8], Command> {
    let (remaining, command_bytes) = command_bytes(input)?;
    let command = match command_bytes {
        b"get" | b"GET" => Command::Get,
        b"set" | b"SET" => Command::Set,
        _ => {
            return Err(nom::Err::Failure((input, nom::error::ErrorKind::Tag)));
        }
    };
    Ok((remaining, command))
}

#[cfg(test)]
mod tests {
    use super::*;
    use nom::Needed;

    #[test]
    fn parse_command_bytes() {
        // as long as we have enough bytes in the buffer, we can parse the
        // entire command
        assert_eq!(
            command_bytes(b"get key\r\n"),
            Ok((&b" key\r\n"[..], &b"get"[..]))
        );
        assert_eq!(command_bytes(b"get "), Ok((&b" "[..], &b"get"[..])));
        assert_eq!(command_bytes(b"get "), Ok((&b" "[..], &b"get"[..])));
        assert_eq!(command_bytes(b"quit\r\n"), Ok((&b"\r\n"[..], &b"quit"[..])));

        // however, if we don't have a non-alphanumeric character, we don't know
        // that the command token is complete
        assert_eq!(
            command_bytes(b"get"),
            Err(nom::Err::Incomplete(Needed::Size(1)))
        );
    }

    #[test]
    fn parse_command() {
        assert_eq!(
            command(b"get key\r\n"),
            Ok((&b" key\r\n"[..], Command::Get))
        );
        assert_eq!(command(b"get "), Ok((&b" "[..], Command::Get)));
        assert_eq!(command(b"GET "), Ok((&b" "[..], Command::Get)));

        assert_eq!(
            command(b"set key \"value\"\r\n"),
            Ok((&b" key \"value\"\r\n"[..], Command::Set))
        );
    }
}
