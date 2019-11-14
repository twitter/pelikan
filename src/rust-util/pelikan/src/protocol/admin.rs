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
use pelikan_sys::protocol::admin::{
    admin_compose_req, admin_compose_rsp, admin_parse_req, admin_request_reset,
    admin_response_reset, request, response, COMPOSE_ENOMEM, COMPOSE_EOVERSIZED, PARSE_EINVALID,
    PARSE_EUNFIN, PARSE_OK, REQ_QUIT,
};

use std::convert::Infallible;
use std::error::Error;
use std::fmt::{self, Display, Formatter};

pub enum AdminProtocol {}

#[derive(Default)]
#[repr(transparent)]
pub struct Request(pub request);

#[derive(Default)]
#[repr(transparent)]
pub struct Response(pub response);

#[derive(Debug)]
pub enum ParseError {
    Unfinished,
    Invalid,
    Other,
}

#[derive(Debug)]
pub enum ComposeError {
    NoMem,
    Oversized,
    Other,
}

impl Protocol for AdminProtocol {
    type Request = Request;
    type Response = Response;
}

impl Serializable for Request {
    type ParseError = ParseError;
    type ComposeError = ComposeError;

    fn reset(&mut self) {
        unsafe { admin_request_reset(&mut self.0 as *mut _) }
    }

    fn parse(&mut self, buf: &mut OwnedBuf) -> Result<(), Self::ParseError> {
        unsafe {
            let status = admin_parse_req(&mut self.0 as *mut _, buf.as_mut_ptr());

            match status {
                PARSE_OK => (),
                PARSE_EUNFIN => return Err(ParseError::Unfinished),
                PARSE_EINVALID => return Err(ParseError::Invalid),
                _ => return Err(ParseError::Other),
            }

            Ok(())
        }
    }
    fn compose(&self, buf: &mut OwnedBuf) -> Result<usize, Self::ComposeError> {
        unsafe {
            let status = admin_compose_req(
                // Not sure what's the proper pattern here
                buf as *mut OwnedBuf as *mut *mut buf,
                // This should actually have been const on the pelikan side
                &self.0 as *const _ as *mut _,
            );

            match status {
                amt if amt >= 0 => Ok(amt as usize),
                COMPOSE_ENOMEM => Err(ComposeError::NoMem),
                COMPOSE_EOVERSIZED => Err(ComposeError::Oversized),
                _ => Err(ComposeError::Other),
            }
        }
    }
}
impl Serializable for Response {
    type ParseError = Infallible;
    type ComposeError = ComposeError;

    fn reset(&mut self) {
        unsafe { admin_response_reset(&mut self.0 as *mut _) }
    }

    fn parse(&mut self, _: &mut OwnedBuf) -> Result<(), Self::ParseError> {
        unimplemented!()
    }
    fn compose(&self, buf: &mut OwnedBuf) -> Result<usize, Self::ComposeError> {
        unsafe {
            let status = admin_compose_rsp(
                buf as *mut OwnedBuf as *mut *mut buf,
                // This should actually have been const on the pelikan side
                &self.0 as *const _ as *mut _,
            );

            match status {
                amt if amt >= 0 => Ok(amt as usize),
                COMPOSE_ENOMEM => Err(ComposeError::NoMem),
                COMPOSE_EOVERSIZED => Err(ComposeError::Oversized),
                _ => Err(ComposeError::Other),
            }
        }
    }
}

impl QuitRequest for Request {
    fn is_quit(&self) -> bool {
        self.0.type_ == REQ_QUIT
    }
}

impl Display for ParseError {
    fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
        match self {
            ParseError::Unfinished => write!(fmt, "EUNFIN"),
            ParseError::Invalid => write!(fmt, "EINVALID"),
            ParseError::Other => write!(fmt, "EOTHER"),
        }
    }
}
impl Error for ParseError {}
impl PartialParseError for ParseError {
    fn is_unfinished(&self) -> bool {
        match self {
            ParseError::Unfinished => true,
            _ => false,
        }
    }
}

impl Display for ComposeError {
    fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
        match self {
            ComposeError::NoMem => write!(fmt, "ENOMEM"),
            ComposeError::Oversized => write!(fmt, "EOVERSIZED"),
            ComposeError::Other => write!(fmt, "EOTHER"),
        }
    }
}
impl Error for ComposeError {}
