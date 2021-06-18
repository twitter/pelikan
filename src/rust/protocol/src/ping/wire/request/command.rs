// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::ParseError;
use core::convert::TryFrom;

/// Ping protocol commands
pub enum PingCommand {
    Ping,
}

impl TryFrom<&[u8]> for PingCommand {
    type Error = ParseError;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        let cmd = match value {
            b"ping" | b"PING" => Self::Ping,
            _ => {
                return Err(ParseError::UnknownCommand);
            }
        };
        Ok(cmd)
    }
}
