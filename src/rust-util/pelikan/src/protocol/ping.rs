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

use std::error::Error;
use std::fmt;
use std::io::{Read, Write};

const REQUEST: &[u8] = b"PING\r\n";
const RESPONSE: &[u8] = b"PONG\r\n";

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

impl StatefulProtocol for PingProtocol {
    type RequestState = ();
    type ResponseState = ();
}

impl<'de> Protocol<'de> for PingProtocol {
    type Request = Request;
    type Response = Response;

    type ParseError = ParseError;
    type ComposeError = ComposeError;

    fn parse_req(_: &mut (), buf: &'de mut OwnedBuf) -> Result<Request, ParseError> {
        if buf.read_size() < REQUEST.len() {
            return Err(ParseError::Unfinished);
        }

        let mut readbuf = [0; REQUEST.len()];
        buf.read_exact(&mut readbuf)
            .map_err(|_| ParseError::Other)?;

        if readbuf == REQUEST {
            Ok(Request)
        } else {
            Err(ParseError::Other)
        }
    }

    fn parse_rsp(_: &mut (), buf: &'de mut OwnedBuf) -> Result<Response, ParseError> {
        if buf.read_size() < RESPONSE.len() {
            return Err(ParseError::Unfinished);
        }

        let mut readbuf = [0; RESPONSE.len()];
        buf.read_exact(&mut readbuf)
            .map_err(|_| ParseError::Other)?;

        if readbuf == RESPONSE {
            Ok(Response)
        } else {
            Err(ParseError::Other)
        }
    }

    fn compose_req(_: Request, _: &mut (), buf: &mut OwnedBuf) -> Result<usize, ComposeError> {
        if buf.write_size() < REQUEST.len() {
            buf.fit(REQUEST.len() - buf.write_size())
                .map_err(|_| ComposeError::NoMem)?;
        }

        buf.write_all(REQUEST)
            .map_err(|_| ComposeError::Other)
            .map(|_| REQUEST.len())
    }

    fn compose_rsp(_: Response, _: &mut (), buf: &mut OwnedBuf) -> Result<usize, ComposeError> {
        if buf.write_size() < RESPONSE.len() {
            buf.fit(RESPONSE.len() - buf.write_size())
                .map_err(|_| ComposeError::NoMem)?;
        }

        buf.write_all(RESPONSE)
            .map_err(|_| ComposeError::Other)
            .map(|_| RESPONSE.len())
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
