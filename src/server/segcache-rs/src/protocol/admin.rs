// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! The memcache admin protocol

// TODO(bmartin): we will replace the admin protocol and listener with a HTTP
// listener in the future.

use crate::buffer::Buffer;
use crate::protocol::CRLF;
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
pub fn parse(buffer: &mut Buffer) -> Result<Request, ParseError> {
    let buf: &[u8] = (*buffer.inner).borrow();

    // check if we got a CRLF
    let mut double_byte_windows = buf.windows(CRLF.len());
    if let Some(command_end) = double_byte_windows.position(|w| w == CRLF.as_bytes()) {
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
                    let _ = buffer.split_to(command_end + CRLF.len());
                    Ok(Request::Stats)
                }
                b"quit" => {
                    let _ = buffer.split_to(command_end + CRLF.len());
                    Ok(Request::Quit)
                }
                b"version" => {
                    let _ = buffer.split_to(command_end + CRLF.len());
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
        for buffer in buffers.iter() {
            let mut b = Buffer::with_capacity(1024);
            b.extend(*buffer);
            assert_eq!(parse(&mut b), Err(ParseError::Incomplete));
        }
    }

    #[test]
    fn parse_stats() {
        let mut b = Buffer::with_capacity(1024);
        b.extend(b"stats\r\n");
        let parsed = parse(&mut b);
        assert!(parsed.is_ok());
        assert_eq!(parsed, Ok(Request::Stats))
    }

    #[test]
    fn parse_quit() {
        let mut b = Buffer::with_capacity(1024);
        b.extend(b"quit\r\n");
        let parsed = parse(&mut b);
        assert!(parsed.is_ok());
        assert_eq!(parsed, Ok(Request::Quit))
    }
}