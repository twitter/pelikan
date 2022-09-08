// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::*;
use common::expiry::TimeType;
use core::fmt::{Display, Formatter};
use protocol_common::{BufMut, Parse, ParseOk};
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

// response codes for klog
const MISS: u8 = 0;
const HIT: u8 = 4;
const STORED: u8 = 5;
const EXISTS: u8 = 6;
const DELETED: u8 = 7;
const NOT_FOUND: u8 = 8;
const NOT_STORED: u8 = 9;

fn string_key(key: &[u8]) -> Cow<'_, str> {
    String::from_utf8_lossy(key)
}

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
    fn parse(&self, buffer: &[u8]) -> Result<ParseOk<Request>, std::io::Error> {
        match self.parse_request(buffer) {
            Ok((input, request)) => Ok(ParseOk::new(request, buffer.len() - input.len())),
            Err(Err::Incomplete(_)) => Err(std::io::Error::from(std::io::ErrorKind::WouldBlock)),
            Err(_) => Err(std::io::Error::from(std::io::ErrorKind::InvalidInput)),
        }
    }
}

impl Compose for Request {
    fn compose(&self, session: &mut dyn BufMut) -> usize {
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

impl Klog for Request {
    type Response = Response;

    fn klog(&self, response: &Self::Response) {
        match self {
            Self::Add(r) => r.klog(response),
            Self::Append(r) => r.klog(response),
            Self::Cas(r) => r.klog(response),
            Self::Decr(r) => r.klog(response),
            Self::Delete(r) => r.klog(response),
            Self::FlushAll(r) => r.klog(response),
            Self::Incr(r) => r.klog(response),
            Self::Get(r) => r.klog(response),
            Self::Gets(r) => r.klog(response),
            Self::Prepend(r) => r.klog(response),
            Self::Quit(r) => r.klog(response),
            Self::Replace(r) => r.klog(response),
            Self::Set(r) => r.klog(response),
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

fn convert_ttl(ttl: Option<u32>) -> Vec<u8> {
    match ttl {
        None => " 0".to_owned(),
        Some(0) => " -1".to_owned(),
        Some(s) => {
            format!(" {}", s)
        }
    }
    .into_bytes()
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
