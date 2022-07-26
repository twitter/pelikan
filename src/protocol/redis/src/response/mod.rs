// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::*;
use protocol_common::*;

mod array;
mod bulk_string;
mod error;
mod integer;
mod simple_string;

pub use array::Array;
pub use bulk_string::BulkString;
pub use error::Error;
pub use integer::Integer;
pub use simple_string::SimpleString;

#[derive(Debug, PartialEq, Eq)]
pub enum Response {
    BulkString(BulkString),
    SimpleString(SimpleString),
    Error(Error),
    Integer(Integer),
    Array(Array),
}

impl Response {
    pub fn simple_string<T: ToString>(string: T) -> Self {
        Self::SimpleString(SimpleString {
            inner: string.to_string(),
        })
    }

    pub fn error<T: ToString>(string: T) -> Self {
        Self::Error(Error {
            inner: string.to_string(),
        })
    }

    pub fn integer(value: u64) -> Self {
        Self::Integer(Integer { inner: value })
    }

    pub fn null() -> Self {
        Self::BulkString(BulkString { inner: None })
    }

    pub fn bulk_string(value: &[u8]) -> Self {
        Self::BulkString(BulkString {
            inner: Some(value.to_vec().into_boxed_slice()),
        })
    }
}

impl Compose for Response {
    fn compose(&self, session: &mut session::Session) {
        match self {
            Self::SimpleString(s) => s.compose(session),
            Self::BulkString(s) => s.compose(session),
            Self::Error(e) => e.compose(session),
            Self::Integer(i) => i.compose(session),
            Self::Array(a) => a.compose(session),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum ResponseType {
    SimpleString,
    Error,
    Integer,
    BulkString,
    Array,
}

pub struct ResponseParser {}

pub(crate) fn response_type(input: &[u8]) -> IResult<&[u8], ResponseType> {
    let (remaining, response_type_token) = take(1usize)(input)?;
    let response_type = match response_type_token {
        b"+" => ResponseType::SimpleString,
        b"-" => ResponseType::Error,
        b":" => ResponseType::Integer,
        b"$" => ResponseType::BulkString,
        b"*" => ResponseType::Array,
        _ => {
            return Err(nom::Err::Failure((input, nom::error::ErrorKind::Tag)));
        }
    };
    Ok((remaining, response_type))
}

pub(crate) fn response(input: &[u8]) -> IResult<&[u8], Response> {
    match response_type(input)? {
        (input, ResponseType::SimpleString) => {
            let (input, response) = simple_string::parse(input)?;
            Ok((input, Response::SimpleString(response)))
        }
        (input, ResponseType::Error) => {
            let (input, response) = error::parse(input)?;
            Ok((input, Response::Error(response)))
        }
        (input, ResponseType::Integer) => {
            let (input, response) = integer::parse(input)?;
            Ok((input, Response::Integer(response)))
        }
        (input, ResponseType::BulkString) => {
            let (input, response) = bulk_string::parse(input)?;
            Ok((input, Response::BulkString(response)))
        }
        (input, ResponseType::Array) => {
            let (input, response) = array::parse(input)?;
            Ok((input, Response::Array(response)))
        }
    }
}

impl Parse<Response> for ResponseParser {
    fn parse(&self, buffer: &[u8]) -> Result<ParseOk<Response>, protocol_common::ParseError> {
        match response(buffer) {
            Ok((input, response)) => Ok(ParseOk::new(response, buffer.len() - input.len())),
            Err(Err::Incomplete(_)) => Err(ParseError::Incomplete),
            Err(_) => Err(ParseError::Invalid),
        }
    }
}
