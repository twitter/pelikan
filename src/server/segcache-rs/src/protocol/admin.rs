// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! The memcache admin protocol

// TODO(bmartin): we will replace the admin protocol and listener with a HTTP
// listener in the future.

use crate::protocol::CRLF;
use crate::protocol::*;

// TODO(bmartin): see TODO for protocol::data::Request, this is cleaner here
// since the variants are simple, but better to take the same approach in both
// modules.
#[derive(PartialEq, Eq, Debug)]
pub enum AdminRequest {
    Stats,
    Version,
    Quit,
}

impl Parse for AdminRequest {
    fn parse(buffer: &[u8]) -> Result<ParseOk<Self>, ParseError> {
        // check if we got a CRLF
        let mut double_byte_windows = buffer.windows(CRLF.len());
        if let Some(command_end) = double_byte_windows.position(|w| w == CRLF.as_bytes()) {
            // single-byte windowing to find spaces
            let mut single_byte_windows = buffer.windows(1);
            if let Some(command_verb_end) = single_byte_windows.position(|w| w == b" ") {
                let command_verb = &buffer[0..command_verb_end];
                // TODO(bmartin): 'stats slab' will go here eventually which will
                // remove the need for ignoring this lint.
                #[allow(clippy::match_single_binding)]
                match command_verb {
                    _ => Err(ParseError::UnknownCommand),
                }
            } else {
                match &buffer[0..command_end] {
                    b"stats" => Ok(ParseOk {
                        message: Self::Stats,
                        consumed: command_end + CRLF.len(),
                    }),
                    b"quit" => Ok(ParseOk {
                        message: Self::Quit,
                        consumed: command_end + CRLF.len(),
                    }),
                    b"version" => Ok(ParseOk {
                        message: Self::Version,
                        consumed: command_end + CRLF.len(),
                    }),
                    _ => Err(ParseError::UnknownCommand),
                }
            }
        } else {
            Err(ParseError::Incomplete)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_incomplete() {
        let buffers: Vec<&[u8]> = vec![b"", b"stats", b"stats\r"];
        for buffer in buffers.iter() {
            assert_eq!(AdminRequest::parse(buffer), Err(ParseError::Incomplete));
        }
    }

    #[test]
    fn parse_quit() {
        let parsed = AdminRequest::parse(b"quit\r\n");
        assert!(parsed.is_ok());
        assert_eq!(parsed.unwrap().into_inner(), AdminRequest::Quit);
    }

    #[test]
    fn parse_stats() {
        let parsed = AdminRequest::parse(b"stats\r\n");
        assert!(parsed.is_ok());
        assert_eq!(parsed.unwrap().into_inner(), AdminRequest::Stats);
    }

    #[test]
    fn parse_version() {
        let parsed = AdminRequest::parse(b"version\r\n");
        assert!(parsed.is_ok());
        assert_eq!(parsed.unwrap().into_inner(), AdminRequest::Version);
    }
}
