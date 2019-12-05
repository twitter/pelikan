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

use std::error::Error;
use std::fmt;

/// A socket address could not be parsed properly.
#[derive(Debug)]
pub enum AddrParseError {
    InvalidIP(std::net::AddrParseError),
    InvalidPort(u64),
}

impl From<std::net::AddrParseError> for AddrParseError {
    fn from(x: std::net::AddrParseError) -> Self {
        Self::InvalidIP(x)
    }
}

impl fmt::Display for AddrParseError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        use self::AddrParseError::*;

        match &self {
            InvalidIP(err) => write!(fmt, "Invalid IP address: {}", err),
            InvalidPort(port) => write!(fmt, "{} is not a valid port number", port),
        }
    }
}

impl Error for AddrParseError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        use self::AddrParseError::*;

        match &self {
            InvalidIP(err) => Some(err),
            _ => None,
        }
    }
}
