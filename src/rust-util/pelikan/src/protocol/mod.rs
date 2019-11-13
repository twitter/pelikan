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

use std::error::Error;

use ccommon::buf::OwnedBuf;

/// An error that could indicate that there weren't enough
/// bytes to successfully parse the buffer.
pub trait PartialParseError {
    /// Indicates that this error occurred because there
    /// weren't enough bytes in the buffer.
    fn is_unfinished(&self) -> bool;
}

pub trait QuitResponse {
    fn is_quit(&self) -> bool;
}

/// A type that can be serialized/deserialized.
///
/// TODO: Name this better
pub trait Serializable: Sized {
    type ParseError: Error + PartialParseError;
    type ComposeError: Error;

    fn reset(&mut self);

    fn parse(&mut self, buf: &mut OwnedBuf) -> Result<(), Self::ParseError>;
    fn compose(&self, buf: &mut OwnedBuf) -> Result<usize, Self::ComposeError>;
}

/// Trait defining the request and response types for a protocol
pub trait Protocol {
    type Request: Serializable + Default;
    type Response: Serializable + Default;
}


/// Useful for cases where stuff can never fail.
/// 
/// This should help with dead-code elimination.
impl PartialParseError for std::convert::Infallible {
    fn is_unfinished(&self) -> bool {
        match *self {}
    }
}
