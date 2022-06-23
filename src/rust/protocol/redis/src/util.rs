// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

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

// consumes zero or more literal spaces
pub fn space0(input: &[u8]) -> IResult<&[u8], &[u8]> {
    input.split_at_position(|item| {
        let c = item.as_char();
        c != ' '
    })
}

// parses a string that is binary safe if wrapped in quotes, otherwise it must
// not contain a space, carriage return, or newline
pub fn string(input: &[u8], max_len: usize) -> IResult<&[u8], &[u8]> {
    match input.get(0) {
        Some(b'\"') => {
            let (input, _) = take(1usize)(input)?;
            let (input, key) = match take_till(|b| b == b'\"')(input) {
                Ok((input, string)) => {
                    if string.is_empty() || string.len() > max_len {
                        Err(nom::Err::Failure((input, nom::error::ErrorKind::Tag)))
                    } else {
                        Ok((input, string))
                    }
                }
                Err(e) => {
                    if input.len() > max_len {
                        Err(nom::Err::Failure((input, nom::error::ErrorKind::Tag)))
                    } else {
                        Err(e)
                    }
                }
            }?;
            let (input, _) = take(1usize)(input)?;
            Ok((input, key))
        }
        Some(_) => match take_till(|b| (b == b' ' || b == b'\r' || b == b'\n'))(input) {
            Ok((input, string)) => {
                if string.is_empty() || string.len() > max_len {
                    Err(nom::Err::Failure((input, nom::error::ErrorKind::Tag)))
                } else {
                    Ok((input, string))
                }
            }
            Err(e) => {
                if input.len() > max_len {
                    Err(nom::Err::Failure((input, nom::error::ErrorKind::Tag)))
                } else {
                    Err(e)
                }
            }
        },
        None => Err(Err::Incomplete(Needed::Size(1))),
    }
}

pub fn parse_u64(input: &[u8]) -> IResult<&[u8], u64> {
    let (input, value) = digit1(input)?;

    // SAFETY: we only matched digits [0-9] to produce the byte
    // slice being transformed to a str here
    let value = unsafe { std::str::from_utf8_unchecked(value) };

    let value = value
        .parse::<u64>()
        .map_err(|_| nom::Err::Failure((input, nom::error::ErrorKind::Tag)))?;
    Ok((input, value))
}
