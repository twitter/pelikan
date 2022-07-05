// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::*;
use common::expiry::TimeType;
use core::fmt::{Display, Formatter};
use protocol_common::Parse;
use protocol_common::{ParseError, ParseOk};
use session::Session;
use std::borrow::Cow;

mod add;
mod append;
mod cas;
mod decr;
mod delete;
mod flush_all;
mod get;
mod gets;
mod incr;
mod prepend;
mod quit;
mod replace;
mod set;

pub use add::Add;
pub use append::Append;
pub use cas::Cas;
pub use decr::Decr;
pub use delete::Delete;
pub use flush_all::FlushAll;
pub use get::Get;
pub use gets::Gets;
pub use incr::Incr;
pub use prepend::Prepend;
pub use quit::Quit;
pub use replace::Replace;
pub use set::Set;

pub const DEFAULT_MAX_BATCH_SIZE: usize = 1024;
pub const DEFAULT_MAX_KEY_LEN: usize = 250;
pub const DEFAULT_MAX_VALUE_SIZE: usize = 512 * 1024 * 1024; // 512MB max value size

#[derive(Copy, Clone)]
pub struct RequestParser {
    max_value_size: usize,
    max_batch_size: usize,
    max_key_len: usize,
    time_type: TimeType,
}

impl RequestParser {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn time_type(mut self, time_type: TimeType) -> Self {
        self.time_type = time_type;
        self
    }

    pub fn max_value_size(mut self, bytes: usize) -> Self {
        self.max_value_size = bytes;
        self
    }

    pub fn max_key_len(mut self, bytes: usize) -> Self {
        self.max_key_len = bytes;
        self
    }

    pub fn max_batch_size(mut self, count: usize) -> Self {
        self.max_batch_size = count;
        self
    }

    fn parse_command<'a>(&self, input: &'a [u8]) -> IResult<&'a [u8], Command> {
        let (remaining, command_bytes) = take_till(|b| (b == b' ' || b == b'\r'))(input)?;
        let command = match command_bytes {
            b"add" | b"ADD" => Command::Add,
            b"append" | b"APPEND" => Command::Append,
            b"cas" | b"CAS" => Command::Cas,
            b"decr" | b"DECR" => Command::Decr,
            b"delete" | b"DELETE" => Command::Delete,
            b"flush_all" | b"FLUSH_ALL" => Command::FlushAll,
            b"incr" | b"INCR" => Command::Incr,
            b"get" | b"GET" => Command::Get,
            b"gets" | b"GETS" => Command::Gets,
            b"prepend" | b"PREPEND" => Command::Prepend,
            b"quit" | b"QUIT" => Command::Quit,
            b"replace" | b"REPLACE" => Command::Replace,
            b"set" | b"SET" => Command::Set,
            _ => {
                // TODO(bmartin): we can return an unknown command error here
                return Err(nom::Err::Failure((input, nom::error::ErrorKind::Tag)));
            }
        };
        Ok((remaining, command))
    }

    pub fn parse_request<'a>(&self, input: &'a [u8]) -> IResult<&'a [u8], Request> {
        match self.parse_command(input)? {
            (input, Command::Add) => {
                let (input, request) = self.parse_add(input)?;
                Ok((input, Request::Add(request)))
            }
            (input, Command::Append) => {
                let (input, request) = self.parse_append(input)?;
                Ok((input, Request::Append(request)))
            }
            (input, Command::Cas) => {
                let (input, request) = self.parse_cas(input)?;
                Ok((input, Request::Cas(request)))
            }
            (input, Command::Decr) => {
                let (input, request) = self.parse_decr(input)?;
                Ok((input, Request::Decr(request)))
            }
            (input, Command::Delete) => {
                let (input, request) = self.parse_delete(input)?;
                Ok((input, Request::Delete(request)))
            }
            (input, Command::FlushAll) => {
                let (input, request) = self.parse_flush_all(input)?;
                Ok((input, Request::FlushAll(request)))
            }
            (input, Command::Incr) => {
                let (input, request) = self.parse_incr(input)?;
                Ok((input, Request::Incr(request)))
            }
            (input, Command::Get) => {
                let (input, request) = self.parse_get(input)?;
                Ok((input, Request::Get(request)))
            }
            (input, Command::Gets) => {
                let (input, request) = self.parse_gets(input)?;
                Ok((input, Request::Gets(request)))
            }
            (input, Command::Prepend) => {
                let (input, request) = self.parse_prepend(input)?;
                Ok((input, Request::Prepend(request)))
            }
            (input, Command::Quit) => {
                let (input, request) = self.parse_quit(input)?;
                Ok((input, Request::Quit(request)))
            }
            (input, Command::Replace) => {
                let (input, request) = self.parse_replace(input)?;
                Ok((input, Request::Replace(request)))
            }
            (input, Command::Set) => {
                let (input, request) = self.parse_set(input)?;
                Ok((input, Request::Set(request)))
            }
        }
    }
}

impl Default for RequestParser {
    fn default() -> Self {
        Self {
            max_value_size: DEFAULT_MAX_VALUE_SIZE,
            max_batch_size: DEFAULT_MAX_BATCH_SIZE,
            max_key_len: DEFAULT_MAX_KEY_LEN,
            time_type: TimeType::Memcache,
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
            Self::Add(r) => r.compose(session),
            Self::Append(r) => r.compose(session),
            Self::Cas(r) => r.compose(session),
            Self::Decr(r) => r.compose(session),
            Self::Delete(r) => r.compose(session),
            Self::FlushAll(r) => r.compose(session),
            Self::Incr(r) => r.compose(session),
            Self::Get(r) => r.compose(session),
            Self::Gets(r) => r.compose(session),
            Self::Prepend(r) => r.compose(session),
            Self::Quit(r) => r.compose(session),
            Self::Replace(r) => r.compose(session),
            Self::Set(r) => r.compose(session),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Request {
    Add(Add),
    Append(Append),
    Cas(Cas),
    Decr(Decr),
    Delete(Delete),
    FlushAll(FlushAll),
    Incr(Incr),
    Get(Get),
    Gets(Gets),
    Prepend(Prepend),
    Quit(Quit),
    Replace(Replace),
    Set(Set),
}

impl Display for Request {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            Request::Add(_) => write!(f, "add"),
            Request::Append(_) => write!(f, "append"),
            Request::Cas(_) => write!(f, "cas"),
            Request::Decr(_) => write!(f, "decr"),
            Request::Delete(_) => write!(f, "delete"),
            Request::FlushAll(_) => write!(f, "flush_all"),
            Request::Incr(_) => write!(f, "incr"),
            Request::Get(_) => write!(f, "get"),
            Request::Gets(_) => write!(f, "gets"),
            Request::Prepend(_) => write!(f, "prepend"),
            Request::Quit(_) => write!(f, "quit"),
            Request::Replace(_) => write!(f, "replace"),
            Request::Set(_) => write!(f, "set"),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Add,
    Append,
    Cas,
    Decr,
    Delete,
    FlushAll,
    Incr,
    Get,
    Gets,
    Prepend,
    Quit,
    Replace,
    Set,
}

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum ExpireTime {
    Seconds(u32),
    UnixSeconds(u32),
}

pub trait Ttl {
    /// The logical view of the TTL (time-to-live). The `None` variant means the
    /// value does not expire. The `Some` variant contains the number of seconds
    /// before the item expires. Zero should be treated as immediate expiration.
    fn ttl(&self) -> Option<u32>;

    /// The wire format view of the TTL (time-to-live). A negative value means
    /// the items should expire immediately. Zero is treated as no-expiry. Any
    /// value >= 1 is the number of seconds until expiration.
    fn ttl_as_i64(&self) -> i64 {
        match self.ttl() {
            None => 0,
            Some(0) => -1,
            Some(t) => t as _,
        }
    }
}

pub trait Keys {
    fn keys(&self) -> &[Box<[u8]>];
}

pub trait Key {
    fn key(&self) -> &[u8];

    fn key_as_str(&self) -> Cow<'_, str> {
        String::from_utf8_lossy(self.key())
    }
}

pub trait NoReply {
    fn noreply(&self) -> bool;
}

pub trait RequestValue {
    fn value(&self) -> &[u8];
}

pub trait Flags {
    fn flags(&self) -> u32;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_command() {
        let parser = RequestParser::new();
        // as long as we have enough bytes in the buffer, we can parse the
        // entire command
        assert!(parser.parse_command(b"get key\r\n").is_ok());
        assert!(parser.parse_command(b"get ").is_ok());
        assert!(parser.parse_command(b"get").is_err());

        assert_eq!(
            parser.parse_command(b"get key\r\n"),
            Ok((&b" key\r\n"[..], Command::Get))
        );
        assert_eq!(parser.parse_command(b"get "), Ok((&b" "[..], Command::Get)));
        assert_eq!(parser.parse_command(b"GET "), Ok((&b" "[..], Command::Get)));

        assert_eq!(
            parser.parse_command(b"set key \"value\"\r\n"),
            Ok((&b" key \"value\"\r\n"[..], Command::Set))
        );
    }
}
