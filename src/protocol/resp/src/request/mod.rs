// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::message::*;
use crate::*;
use protocol_common::BufMut;
use protocol_common::Parse;
use protocol_common::ParseOk;
use std::io::{Error, ErrorKind};
use std::sync::Arc;

mod badd;
mod get;
mod set;

pub use badd::BAddRequest;
pub use get::GetRequest;
pub use set::SetRequest;

#[derive(Default)]
pub struct RequestParser {
    message_parser: MessageParser,
}

impl RequestParser {
    pub fn new() -> Self {
        Self {
            message_parser: MessageParser {},
        }
    }
}

impl Parse<Request> for RequestParser {
    fn parse(&self, buffer: &[u8]) -> Result<ParseOk<Request>, Error> {
        // we have two different parsers, one for RESP and one for inline
        // both require that there's at least one character in the buffer
        if buffer.is_empty() {
            return Err(Error::from(ErrorKind::WouldBlock));
        }

        let (message, consumed) = if matches!(buffer[0], b'*' | b'+' | b'-' | b':' | b'$') {
            self.message_parser.parse(buffer).map(|v| {
                let c = v.consumed();
                (v.into_inner(), c)
            })?
        } else {
            let mut remaining = buffer;

            let mut message = Vec::new();

            while let Ok((r, string)) = string(remaining) {
                message.push(Message::BulkString(BulkString {
                    inner: Some(Arc::new(string.to_owned().into_boxed_slice())),
                }));
                remaining = r;

                if let Ok((r, _)) = space1(remaining) {
                    remaining = r;
                } else {
                    break;
                }
            }

            if &remaining[0..2] != b"\r\n" {
                return Err(Error::from(ErrorKind::WouldBlock));
            }

            let message = Message::Array(Array {
                inner: Some(message),
            });

            let consumed = (buffer.len() - remaining.len()) + 2;

            (message, consumed)
        };

        match &message {
            Message::Array(array) => {
                if array.inner.is_none() {
                    return Err(Error::new(ErrorKind::Other, "malformed command"));
                }

                let array = array.inner.as_ref().unwrap();

                if array.is_empty() {
                    return Err(Error::new(ErrorKind::Other, "malformed command"));
                }

                match &array[0] {
                    Message::BulkString(c) => match c.inner.as_ref().map(|v| v.as_ref().as_ref()) {
                        Some(b"get") | Some(b"GET") => {
                            GetRequest::try_from(message).map(Request::from)
                        }
                        Some(b"set") | Some(b"SET") => {
                            SetRequest::try_from(message).map(Request::from)
                        }
                        Some(b"badd") | Some(b"BADD") => {
                            BAddRequest::try_from(message).map(Request::from)
                        }
                        _ => Err(Error::new(ErrorKind::Other, "unknown command")),
                    },
                    _ => {
                        // all valid commands are encoded as a bulk string
                        Err(Error::new(ErrorKind::Other, "malformed command"))
                    }
                }
            }
            _ => {
                // all valid requests are arrays
                Err(Error::new(ErrorKind::Other, "malformed command"))
            }
        }
        .map(|v| ParseOk::new(v, consumed))
    }
}

impl Compose for Request {
    fn compose(&self, buf: &mut dyn BufMut) -> usize {
        match self {
            Self::Get(r) => r.compose(buf),
            Self::Set(r) => r.compose(buf),
            Self::BAdd(r) => r.compose(buf),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Request {
    Get(GetRequest),
    Set(SetRequest),
    BAdd(BAddRequest),
}

impl From<GetRequest> for Request {
    fn from(other: GetRequest) -> Self {
        Self::Get(other)
    }
}

impl From<SetRequest> for Request {
    fn from(other: SetRequest) -> Self {
        Self::Set(other)
    }
}

impl From<BAddRequest> for Request {
    fn from(other: BAddRequest) -> Self {
        Self::BAdd(other)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Get,
    Set,
    BAdd,
}

impl TryFrom<&[u8]> for Command {
    type Error = ();

    fn try_from(other: &[u8]) -> Result<Self, ()> {
        match other {
            b"get" | b"GET" => Ok(Command::Get),
            b"set" | b"SET" => Ok(Command::Set),
            b"badd" | b"BADD" => Ok(Command::BAdd),
            _ => Err(()),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum ExpireTime {
    Seconds(u64),
    Milliseconds(u64),
    UnixSeconds(u64),
    UnixMilliseconds(u64),
    KeepTtl,
}
