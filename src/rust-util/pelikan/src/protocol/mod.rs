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

/// A type that can be reset to a default state.
pub trait Resettable: Default {
    fn reset(&mut self) {
        *self = Self::default();
    }
}

/// Trait for a native (or otherwise lifetime-independant) protocol.
pub trait Protocol {
    type Request: Resettable;
    type Response: Resettable;

    type ParseError: Error + PartialParseError;
    type ComposeError: Error;

    fn parse_req(state: &mut Self::Request, buf: &mut OwnedBuf) -> Result<(), Self::ParseError>;
    fn parse_rsp(state: &mut Self::Response, buf: &mut OwnedBuf) -> Result<(), Self::ParseError>;

    fn compose_req(req: &Self::Request, buf: &mut OwnedBuf) -> Result<usize, Self::ComposeError>;
    fn compose_rsp(rsp: &Self::Response, buf: &mut OwnedBuf) -> Result<usize, Self::ComposeError>;
}

/// Useful for cases where stuff can never fail.
///
/// This should help with dead-code elimination.
impl PartialParseError for std::convert::Infallible {
    fn is_unfinished(&self) -> bool {
        match *self {}
    }
}
