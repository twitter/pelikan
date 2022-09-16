// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! This module defines all possible `Ping` commands.

use core::convert::TryFrom;

/// Ping response keywords
pub enum Keyword {
    Pong,
}

impl TryFrom<&[u8]> for Keyword {
    type Error = std::io::Error;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        let keyword = match value {
            b"pong" | b"PONG" => Self::Pong,
            _ => {
                return Err(std::io::Error::from(std::io::ErrorKind::InvalidInput));
            }
        };
        Ok(keyword)
    }
}
