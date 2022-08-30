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
pub enum Message {
    BulkString(BulkString),
    SimpleString(SimpleString),
    Error(Error),
    Integer(Integer),
    Array(Array),
}

impl Message {
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

    pub fn integer(value: i64) -> Self {
        Self::Integer(Integer { inner: value })
    }

    pub fn null() -> Self {
        Self::BulkString(BulkString { inner: None })
    }

    pub fn bulk_string(value: &[u8]) -> Self {
        Self::BulkString(BulkString::new(value))
    }
}

impl Compose for Message {
    fn compose(&self, buf: &mut dyn BufMut) -> usize {
        match self {
            Self::SimpleString(s) => s.compose(buf),
            Self::BulkString(s) => s.compose(buf),
            Self::Error(e) => e.compose(buf),
            Self::Integer(i) => i.compose(buf),
            Self::Array(a) => a.compose(buf),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum MessageType {
    SimpleString,
    Error,
    Integer,
    BulkString,
    Array,
}

#[derive(Default)]
pub struct MessageParser {}

pub(crate) fn message_type(input: &[u8]) -> IResult<&[u8], MessageType> {
    let (remaining, response_type_token) = take(1usize)(input)?;
    let response_type = match response_type_token {
        b"+" => MessageType::SimpleString,
        b"-" => MessageType::Error,
        b":" => MessageType::Integer,
        b"$" => MessageType::BulkString,
        b"*" => MessageType::Array,
        _ => {
            return Err(nom::Err::Failure((input, nom::error::ErrorKind::Tag)));
        }
    };
    Ok((remaining, response_type))
}

pub(crate) fn message(input: &[u8]) -> IResult<&[u8], Message> {
    match message_type(input)? {
        (input, MessageType::SimpleString) => {
            let (input, message) = simple_string::parse(input)?;
            Ok((input, Message::SimpleString(message)))
        }
        (input, MessageType::Error) => {
            let (input, message) = error::parse(input)?;
            Ok((input, Message::Error(message)))
        }
        (input, MessageType::Integer) => {
            let (input, message) = integer::parse(input)?;
            Ok((input, Message::Integer(message)))
        }
        (input, MessageType::BulkString) => {
            let (input, message) = bulk_string::parse(input)?;
            Ok((input, Message::BulkString(message)))
        }
        (input, MessageType::Array) => {
            let (input, message) = array::parse(input)?;
            Ok((input, Message::Array(message)))
        }
    }
}

impl Parse<Message> for MessageParser {
    fn parse(&self, buffer: &[u8]) -> Result<ParseOk<Message>, std::io::Error> {
        match message(buffer) {
            Ok((input, message)) => Ok(ParseOk::new(message, buffer.len() - input.len())),
            Err(Err::Incomplete(_)) => Err(std::io::Error::from(std::io::ErrorKind::WouldBlock)),
            Err(_) => Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "malformed message",
            )),
        }
    }
}
