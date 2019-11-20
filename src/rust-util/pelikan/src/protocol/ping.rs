// Copyright (C) 2019 Twitter, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use super::*;
use ccommon::buf::OwnedBuf;
use ccommon_sys::buf;
use pelikan_sys::protocol::ping::*;

use std::error::Error;
use std::fmt;

pub enum PingProtocol {}

#[derive(Copy, Clone, Debug, Default, Eq, Hash, PartialEq)]
pub struct Request;

#[derive(Copy, Clone, Debug, Default, Eq, Hash, PartialEq)]
pub struct Response;

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum ParseError {
    Unfinished,
    Other,
}

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum ComposeError {
    NoMem,
    Other,
}

impl Protocol for PingProtocol {
    type Request = Request;
    type Response = Response;
}

impl Serializable for Request {
    type ParseError = ParseError;
    type ComposeError = ComposeError;

    fn reset(&mut self) {}

    fn parse(&mut self, buf: &mut OwnedBuf) -> Result<(), Self::ParseError> {
        let status = unsafe { parse_req(buf.as_mut_ptr()) };

        match status {
            PARSE_OK => Ok(()),
            PARSE_EUNFIN => Err(ParseError::Unfinished),
            _ => Err(ParseError::Other),
        }
    }

    fn compose(&self, buf: &mut OwnedBuf) -> Result<usize, Self::ComposeError> {
        let status = unsafe { compose_rsp(buf as *mut OwnedBuf as *mut *mut buf) };

        match status {
            COMPOSE_OK => Ok(REQUEST.len()),
            COMPOSE_ENOMEM => Err(ComposeError::NoMem),
            _ => Err(ComposeError::Other),
        }
    }
}

impl Serializable for Response {
    type ParseError = ParseError;
    type ComposeError = ComposeError;

    fn reset(&mut self) {}

    fn parse(&mut self, buf: &mut OwnedBuf) -> Result<(), Self::ParseError> {
        let status = unsafe { parse_rsp(buf.as_mut_ptr()) };

        match status {
            PARSE_OK => Ok(()),
            PARSE_EUNFIN => Err(ParseError::Unfinished),
            _ => Err(ParseError::Other),
        }
    }

    fn compose(&self, buf: &mut OwnedBuf) -> Result<usize, Self::ComposeError> {
        let status = unsafe { compose_rsp(buf as *mut OwnedBuf as *mut *mut buf) };

        match status {
            COMPOSE_OK => Ok(RESPONSE.len()),
            COMPOSE_ENOMEM => Err(ComposeError::NoMem),
            _ => Err(ComposeError::Other),
        }
    }
}

impl Error for ComposeError {}

impl fmt::Display for ComposeError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ComposeError::NoMem => write!(fmt, "ENOMEM"),
            ComposeError::Other => write!(fmt, "EOTHER"),
        }
    }
}

impl Error for ParseError {}

impl fmt::Display for ParseError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ParseError::Unfinished => write!(fmt, "EUNFIN"),
            ParseError::Other => write!(fmt, "EOTHER"),
        }
    }
}

impl PartialParseError for ParseError {
    fn is_unfinished(&self) -> bool {
        match self {
            ParseError::Unfinished => true,
            _ => false,
        }
    }
}
