// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::*;
use protocol_common::Parse;
use protocol_common::{ParseError, ParseOk};
use session::Session;

mod get;
mod set;

pub use get::GetRequest;
pub use set::SetRequest;

pub struct RequestParser {}

impl Parse<Request> for RequestParser {
    fn parse(&self, buffer: &[u8]) -> Result<ParseOk<Request>, protocol_common::ParseError> {
        // we have two different parsers, one for RESP and one for inline
        // both require that there's at least one character in the buffer
        if buffer.is_empty() {
            return Err(ParseError::Incomplete);
        }

        // we can now detect if its a RESP command or inline command
        // all RESP commands are arrays of bulk strings
        let result = match buffer[0] {
            // redis RESP commands must be an array of bulk strings
            b'*' => {
                resp_request(buffer)
            }
            // if the start doesn't match for RESP, it's inline           
            _ => {
                inline_request(buffer)
            }
        };

        match result {
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
            _ => {
                Err(())
            }
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

// A parser for getting the command from an inline request
pub(crate) fn inline_command(input: &[u8]) -> IResult<&[u8], Command> {
    let (remaining, command_bytes) = command_bytes(input)?;
    let command = Command::try_from(command_bytes)
        .map_err(|_| nom::Err::Failure((input, nom::error::ErrorKind::Tag)))?;
    Ok((remaining, command))
}

/// A parser for inline requests, typically used by humans over telnet
pub(crate) fn inline_request(input: &[u8]) -> IResult<&[u8], Request> {
    match inline_command(input)? {
        (input, Command::Get) => {
            get::parse(input).map(|(i, r)| (i, Request::from(r)))
        }
        (input, Command::Set) => {
            set::parse(input).map(|(i, r)| (i, Request::from(r)))
        }
    }
}

/// A parser for RESP formatted requests
pub(crate) fn resp_request(input: &[u8]) -> IResult<&[u8], Request> {
    // all RESP commands are arrays of bulk strings
    // figure out how long the array is
    let (input, _) = char('*')(input)?;
    let (input, alen) = parse_u64(input)?;
    let (mut input, _) = crlf(input)?;

    // empty arrays are invalid
    if alen == 0 {
        return Err(nom::Err::Failure((input, nom::error::ErrorKind::Tag)));
    }
    
    let mut array: Vec<&[u8]> = Vec::with_capacity(alen as usize);

    // loop through the bulk strings and add them to the array
    for _ in 0..alen {
        let (i, _) = char('$')(input)?;
        let (i, len) = parse_u64(i)?;
        let (i, _) = crlf(i)?;
        let (i, string) = take(len as usize)(i)?;
        let (i, _) = crlf(i)?;
        array.push(string);
        input = i;
    }
    
    // figure out which command it is
    let command = Command::try_from(array[0])
        .map_err(|_| nom::Err::Failure((input, nom::error::ErrorKind::Tag)))?;

    // command specific parsing happens here
    let result = match command {
        Command::Get => {
            GetRequest::from_array(&array).map(|v| Request::from(v))
        }
        Command::Set => {
            SetRequest::from_array(&array).map(|v| Request::from(v))
        }
    };

    // map the result and return
    result.map(|v| (input, v)).map_err(|_| nom::Err::Failure((input, nom::error::ErrorKind::Tag)))
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
            inline_command(b"get key\r\n"),
            Ok((&b" key\r\n"[..], Command::Get))
        );
        assert_eq!(inline_command(b"get "), Ok((&b" "[..], Command::Get)));
        assert_eq!(inline_command(b"GET "), Ok((&b" "[..], Command::Get)));

        assert_eq!(
            inline_command(b"set key \"value\"\r\n"),
            Ok((&b" key \"value\"\r\n"[..], Command::Set))
        );
    }
}
