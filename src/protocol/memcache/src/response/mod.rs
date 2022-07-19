// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::*;
use protocol_common::*;
use session_legacy::Session;

mod client_error;
mod deleted;
mod error;
mod exists;
mod not_found;
mod not_stored;
mod numeric;
mod server_error;
mod stored;
mod values;

pub use client_error::ClientError;
pub use deleted::Deleted;
pub use error::Error;
pub use exists::Exists;
pub use not_found::NotFound;
pub use not_stored::NotStored;
pub use numeric::Numeric;
pub use server_error::ServerError;
pub use stored::Stored;
pub use values::{Value, Values};

#[derive(Debug, PartialEq, Eq)]
pub enum Response {
    Error(Error),
    ClientError(ClientError),
    ServerError(ServerError),
    Stored(Stored),
    NotStored(NotStored),
    Exists(Exists),
    NotFound(NotFound),
    Values(Values),
    Numeric(Numeric),
    Deleted(Deleted),
    Hangup,
}

impl Response {
    pub fn error() -> Self {
        Self::Error(Error {})
    }

    pub fn client_error<T: ToString>(string: T) -> Self {
        Self::ClientError(ClientError {
            inner: string.to_string(),
        })
    }

    pub fn server_error<T: ToString>(string: T) -> Self {
        Self::ServerError(ServerError {
            inner: string.to_string(),
        })
    }

    pub fn stored(noreply: bool) -> Self {
        Self::Stored(Stored::new(noreply))
    }

    pub fn not_stored(noreply: bool) -> Self {
        Self::NotStored(NotStored::new(noreply))
    }

    pub fn exists(noreply: bool) -> Self {
        Self::Exists(Exists::new(noreply))
    }

    pub fn not_found(noreply: bool) -> Self {
        Self::NotFound(NotFound::new(noreply))
    }

    pub fn values(values: Box<[Value]>) -> Self {
        Self::Values(Values { values })
    }

    pub fn hangup() -> Self {
        Self::Hangup
    }

    pub fn numeric(value: u64, noreply: bool) -> Self {
        Self::Numeric(Numeric::new(value, noreply))
    }

    pub fn deleted(noreply: bool) -> Self {
        Self::Deleted(Deleted::new(noreply))
    }
}

impl From<Values> for Response {
    fn from(other: Values) -> Self {
        Self::Values(other)
    }
}

impl Compose for Response {
    fn compose(&self, session: &mut dyn BufMut) {
        match self {
            Self::Error(e) => e.compose(session),
            Self::ClientError(e) => e.compose(session),
            Self::ServerError(e) => e.compose(session),
            Self::Stored(e) => e.compose(session),
            Self::NotStored(e) => e.compose(session),
            Self::Exists(e) => e.compose(session),
            Self::NotFound(e) => e.compose(session),
            Self::Values(e) => e.compose(session),
            Self::Numeric(e) => e.compose(session),
            Self::Deleted(e) => e.compose(session),
            Self::Hangup => {}
        }
    }

    fn should_hangup(&self) -> bool {
        matches!(self, Self::Error(_) | Self::ClientError(_) | Self::Hangup)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum ResponseType {
    Error,
    ClientError,
    ServerError,
    Stored,
    NotStored,
    Exists,
    NotFound,
    Values,
    Empty,
    Numeric(u64),
    Deleted,
}

pub struct ResponseParser {}

pub(crate) fn response_type(input: &[u8]) -> IResult<&[u8], ResponseType> {
    let (remaining, response_type_token) = take_till(|b| (b == b' ' || b == b'\r'))(input)?;
    let response_type = match response_type_token {
        b"ERROR" => ResponseType::Error,
        b"CLIENT_ERROR" => ResponseType::ClientError,
        b"SERVER_ERROR" => ResponseType::ServerError,
        b"STORED" => ResponseType::Stored,
        b"NOT_STORED" => ResponseType::NotStored,
        b"EXISTS" => ResponseType::Exists,
        b"NOT_FOUND" => ResponseType::NotFound,
        b"VALUE" => ResponseType::Values,
        b"END" => ResponseType::Empty,
        b"DELETED" => ResponseType::Deleted,
        _ => {
            if let Ok(s) = std::str::from_utf8(response_type_token) {
                if let Ok(value) = s.parse::<u64>() {
                    ResponseType::Numeric(value)
                } else {
                    return Err(nom::Err::Failure((input, nom::error::ErrorKind::Tag)));
                }
            } else {
                return Err(nom::Err::Failure((input, nom::error::ErrorKind::Tag)));
            }
        }
    };
    Ok((remaining, response_type))
}

pub(crate) fn response(input: &[u8]) -> IResult<&[u8], Response> {
    match response_type(input)? {
        (input, ResponseType::Error) => {
            let (input, response) = error::parse(input)?;
            Ok((input, Response::Error(response)))
        }
        (input, ResponseType::ClientError) => {
            let (input, response) = client_error::parse(input)?;
            Ok((input, Response::ClientError(response)))
        }
        (input, ResponseType::ServerError) => {
            let (input, response) = server_error::parse(input)?;
            Ok((input, Response::ServerError(response)))
        }
        (input, ResponseType::Stored) => {
            let (input, response) = stored::parse(input)?;
            Ok((input, Response::Stored(response)))
        }
        (input, ResponseType::NotStored) => {
            let (input, response) = not_stored::parse(input)?;
            Ok((input, Response::NotStored(response)))
        }
        (input, ResponseType::Exists) => {
            let (input, response) = exists::parse(input)?;
            Ok((input, Response::Exists(response)))
        }
        (input, ResponseType::NotFound) => {
            let (input, response) = not_found::parse(input)?;
            Ok((input, Response::NotFound(response)))
        }
        (input, ResponseType::Values) => {
            let (input, response) = values::parse(input)?;
            Ok((input, Response::Values(response)))
        }
        // this is for empty set of values, incidated by "END"
        (input, ResponseType::Empty) => {
            let (input, _) = crlf(input)?;
            Ok((
                input,
                Response::Values(Values {
                    values: Vec::new().into_boxed_slice(),
                }),
            ))
        }
        // this is for numeric responses from incr/decr
        (input, ResponseType::Numeric(value)) => {
            let (input, _) = space0(input)?;
            let (input, _) = crlf(input)?;
            Ok((input, Response::Numeric(Numeric::new(value, false))))
        }
        (input, ResponseType::Deleted) => {
            let (input, response) = deleted::parse(input)?;
            Ok((input, Response::Deleted(response)))
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
