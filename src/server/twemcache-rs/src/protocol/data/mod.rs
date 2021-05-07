// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! The memcache data protocol

use crate::protocol::{CRLF, CRLF_LEN};

use core::convert::TryFrom;

mod command;
mod error;
mod parser;
mod request;
mod response;

// TODO(bmartin):
// - key length checking
// - limit n_keys in get/gets
// - limit total request length for writes (eg - segment size as an upper bound)
//

const NOREPLY: &[u8] = b"noreply";
const NOREPLY_LEN: usize = NOREPLY.len();

pub use command::MemcacheCommand;
pub use error::ParseError;
pub use parser::{MemcacheParser, Parser};
pub use request::{MemcacheRequest, Request};
pub use response::MemcacheResponse;

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::BytesMut;

    fn keys() -> Vec<&'static [u8]> {
        vec![b"0", b"1", b"0123456789", b"A"]
    }

    fn values() -> Vec<&'static [u8]> {
        vec![b"0", b"1", b"0123456789", b"A"]
    }

    fn flags() -> Vec<u32> {
        vec![0, 1, u32::MAX]
    }

    #[test]
    fn parse_incomplete() {
        let buffers: Vec<&[u8]> = vec![
            b"",
            b"get",
            b"get ",
            b"get 0",
            b"get 0\r",
            b"set 0",
            b"set 0 0 0 1",
            b"set 0 0 0 1\r\n",
            b"set 0 0 0 1\r\n1",
            b"set 0 0 0 1\r\n1\r",
            b"set 0 0 0 3\r\n1\r\n\r",
        ];
        for buffer in buffers.iter() {
            let mut b = BytesMut::new();
            b.extend_from_slice(*buffer);
            println!("parse: {:?}", buffer);
            assert_eq!(MemcacheParser::parse(&mut b), Err(ParseError::Incomplete));
        }
    }

    #[test]
    fn parse_get() {
        for key in keys() {
            let mut buffer = BytesMut::new();
            buffer.extend_from_slice(b"get ");
            buffer.extend_from_slice(key);
            buffer.extend_from_slice(b"\r\n");
            println!("parse: {:?}", &buffer);
            let parsed = MemcacheParser::parse(&mut buffer);
            assert!(parsed.is_ok());
            if let Ok(request) = parsed {
                assert_eq!(request.keys().next().unwrap(), key);
            } else {
                panic!("incorrectly parsed");
            }
        }
    }

    #[test]
    fn parse_gets() {
        for key in keys() {
            let mut buffer = BytesMut::new();
            buffer.extend_from_slice(b"gets ");
            buffer.extend_from_slice(key);
            buffer.extend_from_slice(b"\r\n");
            let parsed = MemcacheParser::parse(&mut buffer);
            assert!(parsed.is_ok());
            if let Ok(request) = parsed {
                assert_eq!(request.keys().next().unwrap(), key);
            } else {
                panic!("incorrectly parsed");
            }
        }
    }

    // TODO(bmartin): test multi-get

    #[test]
    fn parse_set() {
        for key in keys() {
            for value in values() {
                for flag in flags() {
                    let mut buffer = BytesMut::new();
                    buffer.extend_from_slice(b"set ");
                    buffer.extend_from_slice(key);
                    buffer.extend_from_slice(format!(" {} 0 {}\r\n", flag, value.len()).as_bytes());
                    buffer.extend_from_slice(value);
                    buffer.extend_from_slice(b"\r\n");
                    let parsed = MemcacheParser::parse(&mut buffer);
                    assert!(parsed.is_ok());
                    if let Ok(request) = parsed {
                        assert_eq!(request.keys().next().unwrap(), key);
                        assert_eq!(request.value(), Some(value));
                        assert_eq!(request.flags(), flag);
                    } else {
                        panic!("incorrectly parsed");
                    }
                }
            }
        }
    }

    // test cases discovered during fuzzing

    #[test]
    // interior newlines and odd spacing for set request
    fn crash_1a() {
        let mut buffer = BytesMut::new();
        buffer.extend_from_slice(b"set 1\r\n0\r\n 0 0   1\r\n0");
        assert!(MemcacheParser::parse(&mut buffer).is_err());
    }

    #[test]
    // interior newlines and odd spacing for add request
    fn crash_1b() {
        let mut buffer = BytesMut::new();
        buffer.extend_from_slice(b"add 1\r\n0\r\n 0 0   1\r\n0");
        assert!(MemcacheParser::parse(&mut buffer).is_err());
    }

    #[test]
    // interior newlines and odd spacing for replace request
    fn crash_1c() {
        let mut buffer = BytesMut::new();
        buffer.extend_from_slice(b"replace 1\r\n0\r\n 0 0   1\r\n0");
        assert!(MemcacheParser::parse(&mut buffer).is_err());
    }

    #[test]
    // interior newlines, odd spacing, null bytes for cas request
    fn crash_2a() {
        let mut buffer = BytesMut::new();
        buffer.extend_from_slice(&[
            0x63, 0x61, 0x73, 0x20, 0x30, 0x73, 0x31, 0x31, 0x31, 0x31, 0x31, 0x31, 0x31, 0x31,
            0x31, 0x31, 0x31, 0x31, 0x31, 0x31, 0x00, 0x00, 0x31, 0x31, 0x31, 0x31, 0x31, 0x31,
            0x31, 0x31, 0x31, 0x31, 0x31, 0x0D, 0x0A, 0x65, 0x74, 0x20, 0x30, 0x20, 0x30, 0x20,
            0x30, 0x20, 0x31, 0x0D, 0x0A, 0x30, 0x0D, 0x0D, 0x0D, 0x0D, 0x0D, 0x0D, 0x1C, 0x0D,
            0x64, 0x65, 0x6C, 0x65, 0x74, 0x65, 0x20, 0x18,
        ]);
        assert!(MemcacheParser::parse(&mut buffer).is_err());
    }

    // TODO(bmartin): add test for add / replace / delete
}
