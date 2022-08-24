// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::message::*;
use crate::*;
use protocol_common::Parse;
use protocol_common::{ParseError, ParseOk};
use session::Session;

mod get;
mod set;

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
    fn parse(&self, buffer: &[u8]) -> Result<ParseOk<Request>, protocol_common::ParseError> {
        // we have two different parsers, one for RESP and one for inline
        // both require that there's at least one character in the buffer
        if buffer.is_empty() {
            return Err(ParseError::Incomplete);
        }

        let (message, consumed) = if matches!(buffer[0], b'*' | b'+' | b'-' | b':' | b'$' ) {
            self.message_parser.parse(buffer).map(|v| {
                let c = v.consumed();
                (v.into_inner(), c)
            })?
        } else {
            let mut remaining = buffer;

            let mut message = Vec::new();

            // build up the array of bulk strings
            loop {
                if let Ok((r, string)) = string(remaining) {
                    message.push(Message::BulkString(BulkString { inner: Some(string.to_owned().into_boxed_slice()) }));
                    remaining = r;
                } else {
                    break;
                }

                if let Ok((r, _)) = space1(remaining) {
                    remaining = r;
                } else {
                    break;
                }
            }

            if &remaining[0..2] != b"\r\n" {
                return Err(ParseError::Incomplete);
            }

            let message = Message::Array(Array { inner: Some(message) });

            let consumed = (buffer.len() - remaining.len()) + 2;

            (message, consumed)
        };


        match &message {
            Message::Array(array) => {
                if array.inner.is_none() {
                    return Err(ParseError::Invalid);
                }

                let array = array.inner.as_ref().unwrap();

                if array.is_empty() {
                    return Err(ParseError::Invalid);
                }

                match &array[0] {
                    Message::BulkString(c) => {
                        match c.inner.as_ref().map(|v| Command::try_from(v.as_ref())) {
                            Some(Ok(Command::Get)) => {
                                GetRequest::try_from(message).map(Request::from)
                            }
                            Some(Ok(Command::Set)) => {
                                SetRequest::try_from(message).map(Request::from)
                            }
                            _ => Err(ParseError::Invalid),
                        }
                    }
                    _ => {
                        // all valid commands are encoded as a bulk string
                        Err(ParseError::Invalid)
                    }
                }
            }
            _ => {
                // all valid requests are arrays
                Err(ParseError::Invalid)
            }
        }
        .map(|v| ParseOk::new(v, consumed))
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
    Get(GetRequest),
    Set(SetRequest),
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

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Get,
    Set,
}

impl<'a> TryFrom<&'a [u8]> for Command {
    type Error = ();

    fn try_from(other: &[u8]) -> Result<Self, ()> {
        match other {
            b"get" | b"GET" => Ok(Command::Get),
            b"set" | b"SET" => Ok(Command::Set),
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

pub(crate) fn command_bytes(input: &[u8]) -> IResult<&[u8], &[u8]> {
    alphanumeric1(input)
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
}
