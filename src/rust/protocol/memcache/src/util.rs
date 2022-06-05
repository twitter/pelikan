// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

pub use nom::bytes::streaming::*;
pub use nom::character::streaming::*;
pub use nom::error::ErrorKind;
use nom::error::ParseError;
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

pub fn signed_digit1<T, E: ParseError<T>>(input: T) -> IResult<T, T, E>
where
    T: InputTakeAtPosition,
    <T as InputTakeAtPosition>::Item: AsChar,
{
    input.split_at_position1(
        |item| {
            let c = item.as_char();
            !c.is_ascii_digit() && c != '-'
        },
        ErrorKind::Digit,
    )
}

// parses a string that is binary safe and less than the max key length
pub fn key(input: &[u8], max_len: usize) -> IResult<&[u8], Option<&[u8]>> {
    let (i, key) = take_till(|b| (b == b' ' || b == b'\r'))(input).map_err(|e| {
        if let nom::Err::Incomplete(_) = e {
            if input.len() > max_len {
                nom::Err::Failure((input, nom::error::ErrorKind::Tag))
            } else {
                e
            }
        } else {
            e
        }
    })?;
    if key.len() > max_len {
        return Err(nom::Err::Failure((input, nom::error::ErrorKind::Tag)));
    }
    if key.len() == 0 {
        // returns unmodified input and signals that no key was found
        Ok((input, None))
    } else {
        // returns the remaining input and the key that was found
        Ok((i, Some(key)))
    }
}

pub fn parse_usize(input: &[u8]) -> IResult<&[u8], usize> {
    let (input, value) = digit1(input)?;

    // SAFETY: we only matched digits [0-9] to produce the byte
    // slice being transformed to a str here
    let value = unsafe { std::str::from_utf8_unchecked(value) };

    let value = value
        .parse::<usize>()
        .map_err(|_| nom::Err::Failure((input, nom::error::ErrorKind::Tag)))?;
    Ok((input, value))
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

pub fn parse_i64(input: &[u8]) -> IResult<&[u8], i64> {
    let (input, value) = signed_digit1(input)?;

    // SAFETY: we only matched digits [0-9] and '-' to produce the byte
    // slice being transformed to a str here
    let value = unsafe { std::str::from_utf8_unchecked(value) };

    let value = value
        .parse::<i64>()
        .map_err(|_| nom::Err::Failure((input, nom::error::ErrorKind::Tag)))?;
    Ok((input, value))
}

pub fn parse_u32(input: &[u8]) -> IResult<&[u8], u32> {
    let (input, value) = digit1(input)?;

    // SAFETY: we only matched digits [0-9] to produce the byte
    // slice being transformed to a str here
    let value = unsafe { std::str::from_utf8_unchecked(value) };

    let value = value
        .parse::<u32>()
        .map_err(|_| nom::Err::Failure((input, nom::error::ErrorKind::Tag)))?;
    Ok((input, value))
}
