// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! This module defines all possible `Ping` commands.

use crate::ParseError;
use core::convert::TryFrom;

/// Ping response keywords
pub enum Keyword {
    Ping,
}

impl TryFrom<&[u8]> for Keyword {
    type Error = ParseError;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        let keyword = match value {
            b"ping" | b"PING" => Self::Ping,
            _ => {
                return Err(ParseError::Unknown);
            }
        };
        Ok(keyword)
    }
}
