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

use core::fmt::{self, Display, Formatter};

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum Empty {}

/// Error types.
///
/// # Note
/// This enum is non-exhaustive, adding new variants is not
/// considered a breaking change.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum Error {
    /// Ran out of buffer space
    OutOfBuffer,
    /// A custom method contained invalid characters
    InvalidMethod,
    /// The URI contained invalid characters
    InvalidUri,
    /// A custom version contains invalid characters
    InvalidVersion,
    /// A header key contained invalid characters
    InvalidHeaderKey,
    /// A header value contained invalid characters
    InvalidHeaderValue,

    #[doc(hidden)]
    __Nonexhaustive(Empty),
}

impl Display for Error {
    fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
        use self::Error::*;

        fmt.write_str(match self {
            OutOfBuffer => "Out of buffer space",
            InvalidMethod => "Invalid HTTP method",
            InvalidUri => "Invalid HTTP Uri",
            InvalidVersion => "Invalid HTTP Version",
            InvalidHeaderKey => "Invalid header key",
            InvalidHeaderValue => "Invalid header value",

            &__Nonexhaustive(empty) => match empty {},
        })
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Error {}
