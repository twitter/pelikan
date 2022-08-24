// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use protocol_common::ParseError;
use crate::message::*;
pub use nom::bytes::streaming::*;
pub use nom::character::streaming::*;
pub use nom::error::ErrorKind;
pub use nom::{AsChar, Err, IResult, InputTakeAtPosition, Needed};
pub use protocol_common::Compose;
pub use std::io::Write;

// consumes one or more literal spaces
pub fn space1(input: &[u8]) -> IResult<&[u8], &[u8]> {
    input.split_at_position1(
        |item| {
            let c = item.as_char();
            c != ' '
        },
        ErrorKind::Space,
    )
}

// parses a string that is binary safe if wrapped in quotes, otherwise it must
// not contain a space, carriage return, or newline
pub fn string(input: &[u8]) -> IResult<&[u8], &[u8]> {
    match input.get(0) {
        Some(b'\"') => {
            let (input, _) = take(1usize)(input)?;
            let (input, key) = take_till(|b| b == b'\"')(input)?;
            let (input, _) = take(1usize)(input)?;
            Ok((input, key))
        }
        Some(_) => take_till(|b| (b == b' ' || b == b'\r' || b == b'\n'))(input),
        None => Err(Err::Incomplete(Needed::Size(1))),
    }
}


pub fn take_bulk_string(array: &mut Vec<Message>) -> Result<Box<[u8]>, ParseError> {
    if let Message::BulkString(s) = array.remove(1) {
        if s.inner.is_none() {
            return Err(ParseError::Invalid);
        }

        let s = s.inner.unwrap();

        Ok(s)
    } else {
        Err(ParseError::Invalid)
    }
}

pub fn take_bulk_string_as_u64(array: &mut Vec<Message>) -> Result<u64, ParseError> {
    let s = take_bulk_string(array)?;
    std::str::from_utf8(&s)
        .map_err(|_| ParseError::Invalid)?
        .parse::<u64>()
        .map_err(|_| ParseError::Invalid)
}