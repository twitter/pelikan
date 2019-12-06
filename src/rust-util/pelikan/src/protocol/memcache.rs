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

use pelikan_sys::protocol::memcache::*;

use std::error::Error;
use std::fmt::{self, Display, Formatter};

pub enum MemcacheProtocol {}

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
#[repr(C)]
#[rustfmt::skip]
pub enum ParseError {
    Unfinished  = PARSE_EUNFIN    as isize,
    Invalid     = PARSE_EINVALID  as isize,
    Oversize    = PARSE_EOVERSIZE as isize,
    Empty       = PARSE_EEMPTY    as isize,
    Other       = PARSE_EOTHER    as isize,
}

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
#[repr(C)]
#[rustfmt::skip]
pub enum ComposeError {
    Invalid     = COMPOSE_EINVALID as isize,
    NoMem       = COMPOSE_ENOMEM   as isize,
    Unfinished  = COMPOSE_EUNFIN   as isize,
    Other       = COMPOSE_EOTHER   as isize,
}

impl Protocol for MemcacheProtocol {
    type Request = request;
    type Response = response;

    type ParseError = ParseError;
    type ComposeError = ComposeError;

    fn parse_req(req: &mut request, buf: &mut OwnedBuf) -> Result<(), ParseError> {
        let status = unsafe { parse_req(req as *mut _, buf.as_mut_ptr()) };

        match status {
            PARSE_OK => Ok(()),
            PARSE_EUNFIN => Err(ParseError::Unfinished),
            PARSE_EINVALID => Err(ParseError::Invalid),
            PARSE_EOVERSIZE => Err(ParseError::Oversize),
            PARSE_EEMPTY => Err(ParseError::Empty),
            _ => Err(ParseError::Other),
        }
    }

    fn parse_rsp(rsp: &mut response, buf: &mut OwnedBuf) -> Result<(), ParseError> {
        let status = unsafe { parse_rsp(rsp as *mut _, buf.as_mut_ptr()) };

        match status {
            PARSE_OK => Ok(()),
            PARSE_EUNFIN => Err(ParseError::Unfinished),
            PARSE_EINVALID => Err(ParseError::Invalid),
            PARSE_EOVERSIZE => Err(ParseError::Oversize),
            PARSE_EEMPTY => Err(ParseError::Empty),
            _ => Err(ParseError::Other),
        }
    }

    fn compose_req(req: &request, buf: &mut OwnedBuf) -> Result<usize, ComposeError> {
        let status =
            unsafe { compose_req(buf as *mut OwnedBuf as *mut *mut buf, req as *const request) };

        match status {
            cnt if cnt >= 0 => Ok(cnt as usize),
            COMPOSE_EUNFIN => Err(ComposeError::Unfinished),
            COMPOSE_EINVALID => Err(ComposeError::Invalid),
            COMPOSE_ENOMEM => Err(ComposeError::NoMem),
            _ => Err(ComposeError::Other),
        }
    }

    fn compose_rsp(req: &response, buf: &mut OwnedBuf) -> Result<usize, ComposeError> {
        let status = unsafe {
            compose_rsp(
                buf as *mut OwnedBuf as *mut *mut buf,
                req as *const response,
            )
        };

        match status {
            cnt if cnt >= 0 => Ok(cnt as usize),
            COMPOSE_EUNFIN => Err(ComposeError::Unfinished),
            COMPOSE_EINVALID => Err(ComposeError::Invalid),
            COMPOSE_ENOMEM => Err(ComposeError::NoMem),
            _ => Err(ComposeError::Other),
        }
    }
}

impl Resettable for request {
    fn reset(&mut self) {
        unsafe { request_reset(self as *mut _) }
    }
}

impl Resettable for response {
    fn reset(&mut self) {
        unsafe { response_reset(self as *mut _) }
    }
}

impl Display for ParseError {
    fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
        match self {
            ParseError::Unfinished => write!(fmt, "EUNFIN"),
            ParseError::Invalid => write!(fmt, "EINVALID"),
            ParseError::Oversize => write!(fmt, "EOVERSIZE"),
            ParseError::Empty => write!(fmt, "EEMPTY"),
            ParseError::Other => write!(fmt, "EOTHER"),
        }
    }
}

impl Display for ComposeError {
    fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
        match self {
            ComposeError::Unfinished => write!(fmt, "EUNFIN"),
            ComposeError::Invalid => write!(fmt, "EINVALID"),
            ComposeError::NoMem => write!(fmt, "ENOMEM"),
            ComposeError::Other => write!(fmt, "EOTHER"),
        }
    }
}

impl Error for ParseError {}

impl Error for ComposeError {}

impl PartialParseError for ParseError {
    fn is_unfinished(&self) -> bool {
        match self {
            ParseError::Unfinished => true,
            _ => false,
        }
    }
}
