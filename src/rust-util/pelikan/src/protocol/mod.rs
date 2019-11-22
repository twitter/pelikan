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

#[cfg(feature = "protocol_admin")]
pub mod admin;
#[cfg(feature = "protocol_ping")]
pub mod ping;

use std::error::Error;

use ccommon::buf::OwnedBuf;

/// An error that could indicate that there weren't enough
/// bytes to successfully parse the buffer.
pub trait PartialParseError {
    /// Indicates that this error occurred because there
    /// weren't enough bytes in the buffer.
    fn is_unfinished(&self) -> bool;
}

pub trait Resettable {
    fn reset(&mut self);
}

pub trait StatefulProtocol {
    type RequestState: Default + Resettable;
    type ResponseState: Default + Resettable;
}

/// Trait defining the request and response types for a protocol
pub trait Protocol<'de>: StatefulProtocol {
    type Request: 'de;
    type Response: 'de;

    type ParseError: Error + PartialParseError + 'de;
    type ComposeError: Error + 'de;

    fn parse_req(
        state: &mut <Self as StatefulProtocol>::RequestState,
        buf: &'de mut OwnedBuf,
    ) -> Result<Self::Request, Self::ParseError>;
    fn parse_rsp(
        state: &mut <Self as StatefulProtocol>::ResponseState,
        buf: &'de mut OwnedBuf,
    ) -> Result<Self::Response, Self::ParseError>;

    fn compose_req(
        req: Self::Request,
        state: &mut <Self as StatefulProtocol>::RequestState,
        buf: &'de mut OwnedBuf,
    ) -> Result<usize, Self::ComposeError>;
    fn compose_rsp(
        rsp: Self::Response,
        state: &mut <Self as StatefulProtocol>::ResponseState,
        buf: &'de mut OwnedBuf,
    ) -> Result<usize, Self::ComposeError>;
}

/// Useful for cases where stuff can never fail.
///
/// This should help with dead-code elimination.
impl PartialParseError for std::convert::Infallible {
    fn is_unfinished(&self) -> bool {
        match *self {}
    }
}
