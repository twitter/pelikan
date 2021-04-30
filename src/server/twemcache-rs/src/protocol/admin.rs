// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::protocol::{CRLF, CRLF_LEN};
use bytes::BytesMut;
use std::borrow::Borrow;

// TODO(bmartin): see TODO for protocol::data::Request, this is cleaner here
// since the variants are simple, but better to take the same approach in both
// modules.
#[derive(PartialEq, Eq, Debug)]
pub enum Request {
    Stats,
    Version,
    Quit,
}

#[derive(PartialEq, Eq, Debug)]
pub enum ParseError {
    Incomplete,
    UnknownCommand,
}

// TODO(bmartin): see corresponding TODO for protocol::data::parse()
pub fn parse(buffer: &mut BytesMut) -> Result<Request, ParseError> {
    // no-copy borrow as a slice
    let buf: &[u8] = (*buffer).borrow();

    // check if we got a CRLF
    let mut double_byte_windows = buf.windows(CRLF_LEN);
    if let Some(command_end) = double_byte_windows.position(|w| w == CRLF) {
        // single-byte windowing to find spaces
        let mut single_byte_windows = buf.windows(1);
        if let Some(command_verb_end) = single_byte_windows.position(|w| w == b" ") {
            let command_verb = &buf[0..command_verb_end];
            // TODO(bmartin): 'stats slab' will go here eventually which will
            // remove the need for ignoring this lint.
            #[allow(clippy::match_single_binding)]
            match command_verb {
                _ => Err(ParseError::UnknownCommand),
            }
        } else {
            match &buf[0..command_end] {
                b"stats" => {
                    let _ = buffer.split_to(command_end + CRLF_LEN);
                    Ok(Request::Stats)
                }
                b"quit" => {
                    let _ = buffer.split_to(command_end + CRLF_LEN);
                    Ok(Request::Quit)
                }
                b"version" => {
                    let _ = buffer.split_to(command_end + CRLF_LEN);
                    Ok(Request::Version)
                }
                _ => Err(ParseError::UnknownCommand),
            }
        }
    } else {
        Err(ParseError::Incomplete)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_incomplete() {
        let buffers: Vec<&[u8]> = vec![b"", b"stats", b"stats\r"];
        for mut buffer in buffers.iter().map(|v| BytesMut::from(&v[..])) {
            assert_eq!(parse(&mut buffer), Err(ParseError::Incomplete));
        }
    }

    #[test]
    fn parse_stats() {
        let mut buffer = BytesMut::new();
        buffer.extend_from_slice(b"stats\r\n");
        let parsed = parse(&mut buffer);
        assert!(parsed.is_ok());
        if let Ok(Request::Stats) = parsed {
        } else {
            panic!("incorrectly parsed");
        }
    }

    #[test]
    fn parse_quit() {
        let mut buffer = BytesMut::new();
        buffer.extend_from_slice(b"quit\r\n");
        let parsed = parse(&mut buffer);
        assert!(parsed.is_ok());
        if let Ok(Request::Quit) = parsed {
        } else {
            panic!("incorrectly parsed");
        }
    }
}
