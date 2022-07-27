// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Implements the `Admin` protocol.

// TODO(bmartin): we will replace the admin protocol and listener with a HTTP
// listener in the future.

use crate::*;
use common::bytes::SliceExtension;

use std::io::{Error, ErrorKind, Result};

// TODO(bmartin): see TODO for protocol::data::Request, this is cleaner here
// since the variants are simple, but better to take the same approach in both
// modules.
#[derive(PartialEq, Eq, Debug)]
pub enum AdminRequest {
    FlushAll,
    Stats,
    Version,
    Quit,
}

#[derive(Default, Copy, Clone)]
pub struct AdminRequestParser {}

impl AdminRequestParser {
    pub fn new() -> Self {
        Self {}
    }
}

impl Parse<AdminRequest> for AdminRequestParser {
    fn parse(&self, buffer: &[u8]) -> Result<ParseOk<AdminRequest>> {
        // check if we got a CRLF
        if let Some(command_end) = buffer
            .windows(CRLF.len())
            .position(|w| w == CRLF.as_bytes())
        {
            let trimmed_buffer = &buffer[0..command_end].trim();

            // single-byte windowing to find spaces
            let mut single_byte_windows = trimmed_buffer.windows(1);
            if let Some(command_verb_end) = single_byte_windows.position(|w| w == b" ") {
                let command_verb = &trimmed_buffer[0..command_verb_end];
                // TODO(bmartin): 'stats slab' will go here eventually which will
                // remove the need for ignoring this lint.
                #[allow(clippy::match_single_binding)]
                match command_verb {
                    _ => Err(Error::from(ErrorKind::InvalidInput)),
                }
            } else {
                match &trimmed_buffer[0..] {
                    b"flush_all" => Ok(ParseOk::new(
                        AdminRequest::FlushAll,
                        command_end + CRLF.len(),
                    )),
                    b"stats" => Ok(ParseOk::new(AdminRequest::Stats, command_end + CRLF.len())),
                    b"quit" => Ok(ParseOk::new(AdminRequest::Quit, command_end + CRLF.len())),
                    b"version" => Ok(ParseOk::new(
                        AdminRequest::Version,
                        command_end + CRLF.len(),
                    )),
                    _ => Err(Error::from(ErrorKind::InvalidInput)),
                }
            }
        } else {
            Err(Error::from(ErrorKind::WouldBlock))
        }
    }
}

pub struct AdminResponse {}

impl Compose for AdminResponse {
    fn compose(&self, _: &mut dyn protocol_common::BufMut) -> usize {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_incomplete() {
        let parser = AdminRequestParser::new();

        let buffers: Vec<&[u8]> = vec![b"", b"stats", b"stats\r"];
        for buffer in buffers.iter() {
            if let Err(e) = parser.parse(buffer) {
                assert_eq!(e.kind(), ErrorKind::WouldBlock);
            } else {
                panic!("parser should not have returned a request");
            }
        }
    }

    #[test]
    fn parse_flush_all() {
        let parser = AdminRequestParser::new();

        let parsed = parser.parse(b"flush_all\r\n");
        assert!(parsed.is_ok());
        assert_eq!(parsed.unwrap().into_inner(), AdminRequest::FlushAll);
    }

    #[test]
    fn parse_quit() {
        let parser = AdminRequestParser::new();

        let parsed = parser.parse(b"quit\r\n");
        assert!(parsed.is_ok());
        assert_eq!(parsed.unwrap().into_inner(), AdminRequest::Quit);
    }

    #[test]
    fn parse_stats() {
        let parser = AdminRequestParser::new();

        let parsed = parser.parse(b"stats\r\n");
        assert!(parsed.is_ok());
        assert_eq!(parsed.unwrap().into_inner(), AdminRequest::Stats);
    }

    #[test]
    fn parse_version() {
        let parser = AdminRequestParser::new();

        let parsed = parser.parse(b"version\r\n");
        assert!(parsed.is_ok());
        assert_eq!(parsed.unwrap().into_inner(), AdminRequest::Version);
    }

    #[test]
    fn parse_commands_with_whitespace_leading_or_trailing() {
        let parser = AdminRequestParser::new();

        let parsed = parser.parse(b"version  \r\n");
        assert!(parsed.is_ok());
        assert_eq!(parsed.unwrap().into_inner(), AdminRequest::Version);

        let parsed = parser.parse(b"  version\r\n");
        assert!(parsed.is_ok());
        assert_eq!(parsed.unwrap().into_inner(), AdminRequest::Version);

        let parsed = parser.parse(b"  quit  \r\n");
        assert!(parsed.is_ok());
        assert_eq!(parsed.unwrap().into_inner(), AdminRequest::Quit);
    }

    #[test]
    fn parse_ignores_after_crlf() {
        let parser = AdminRequestParser::new();

        let parsed = parser.parse(b"flush_all\r\nstats");
        assert!(parsed.is_ok());
        assert_eq!(parsed.unwrap().into_inner(), AdminRequest::FlushAll);
    }
}
