// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::protocol::{CRLF, CRLF_LEN};

use bytes::BytesMut;
use segcache::Item;

use core::convert::TryFrom;
use std::borrow::Borrow;

// TODO(bmartin):
// - key length checking
// - limit n_keys in get/gets
// - limit total request length for writes (eg - segment size as an upper bound)
//

const NOREPLY: &[u8] = b"noreply";
const NOREPLY_LEN: usize = NOREPLY.len();

#[derive(PartialEq, Eq, Debug)]
pub enum ParseError {
    Incomplete,
    Invalid,
    UnknownCommand,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum MemcacheCommand {
    Get,
    Gets,
    Set,
    Add,
    Replace,
    Cas,
    Delete,
}

impl TryFrom<&[u8]> for MemcacheCommand {
    type Error = ParseError;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        let cmd = match value {
            b"get" => MemcacheCommand::Get,
            b"gets" => MemcacheCommand::Gets,
            b"set" => MemcacheCommand::Set,
            b"add" => MemcacheCommand::Add,
            b"replace" => MemcacheCommand::Replace,
            b"cas" => MemcacheCommand::Cas,
            b"delete" => MemcacheCommand::Delete,
            _ => {
                return Err(ParseError::UnknownCommand);
            }
        };
        Ok(cmd)
    }
}

// TODO(bmartin): this should be lifted out into a common crate and shared
// between different protocols
pub trait Request {
    type Command;

    fn command(&self) -> Self::Command;
    fn keys(&self) -> Vec<&[u8]>;
}

#[derive(Debug, PartialEq)]
pub struct MemcacheRequest {
    buffer: BytesMut,
    command: MemcacheCommand,
    consumed: usize,
    keys: Vec<(usize, usize)>,
    noreply: bool,
    expiry: u32,
    flags: u32,
    value: (usize, usize),
    cas: u64,
}

impl MemcacheRequest {
    /// Should a reply be sent to the client for this request?
    pub fn noreply(&self) -> bool {
        self.noreply
    }

    /// Return the number of bytes consumed from the read buffer by the parsed
    /// request
    pub fn consumed(&self) -> usize {
        self.consumed
    }

    /// Return the expiry for the value
    pub fn expiry(&self) -> u32 {
        self.expiry
    }

    pub fn flags(&self) -> u32 {
        self.flags
    }

    pub fn value(&self) -> Option<&[u8]> {
        let start = self.value.0;
        let end = self.value.1;
        if start == end {
            None
        } else {
            Some(&self.buffer[start..end])
        }
    }

    pub fn cas(&self) -> u64 {
        self.cas
    }
}

impl Request for MemcacheRequest {
    type Command = MemcacheCommand;

    fn command(&self) -> Self::Command {
        self.command
    }

    fn keys(&self) -> Vec<&[u8]> {
        let buffer: &[u8] = self.buffer.borrow();
        let mut keys = Vec::new();
        for key_index in &self.keys {
            keys.push(&buffer[key_index.0..key_index.1])
        }
        keys
    }
}

pub struct MemcacheParser;

// TODO(bmartin): this should be lifted out into a common crate and shared
// between different protocols
pub trait Parser {
    type Request: Request;

    fn parse(buffer: &mut BytesMut) -> Result<Self::Request, ParseError>;
}

impl Parser for MemcacheParser {
    type Request = MemcacheRequest;
    
    fn parse(buffer: &mut BytesMut) -> Result<Self::Request, ParseError> {
        let command;
        {
            // no-copy borrow as a slice
            let buf: &[u8] = (*buffer).borrow();
            // check if we got a CRLF
            let mut double_byte = buf.windows(CRLF_LEN);
            if let Some(_line_end) = double_byte.position(|w| w == CRLF) {
                // single-byte windowing to find spaces
                let mut single_byte = buf.windows(1);
                if let Some(cmd_end) = single_byte.position(|w| w == b" ") {
                    command = MemcacheCommand::try_from(&buf[0..cmd_end])?;
                } else {
                    return Err(ParseError::Incomplete);
                }
            } else {
                return Err(ParseError::Incomplete);
            }
        }

        match command {
            MemcacheCommand::Get => Self::parse_get(buffer),
            MemcacheCommand::Gets => Self::parse_gets(buffer),
            MemcacheCommand::Set => Self::parse_set(buffer),
            MemcacheCommand::Add => Self::parse_add(buffer),
            MemcacheCommand::Replace => Self::parse_replace(buffer),
            MemcacheCommand::Cas => Self::parse_cas(buffer),
            MemcacheCommand::Delete => Self::parse_delete(buffer),
        }
    }
}

impl MemcacheParser {
    // by the time we call this, we know we have a CRLF and spaces and a valid
    // command name
    #[allow(clippy::unnecessary_wraps)]
    fn parse_get(buffer: &mut BytesMut) -> Result<MemcacheRequest, ParseError> {
        let buf: &[u8] = (*buffer).borrow();

        let mut double_byte = buf.windows(2);
        let line_end = double_byte.position(|w| w == CRLF).unwrap();

        let mut single_byte = buf.windows(1);
        // we already checked for this in the MemcacheParser::parse()
        let cmd_end = single_byte.position(|w| w == b" ").unwrap();
        let mut previous = cmd_end + 1;
        let mut keys = Vec::new();

        // command may have multiple keys, we need to loop until we hit
        // a CRLF
        loop {
            if let Some(key_end) = single_byte.position(|w| w == b" ") {
                if key_end < line_end {
                    keys.push((previous, key_end));
                    previous = key_end + 1;
                } else {
                    keys.push((previous, line_end));
                    break;
                }
            } else {
                keys.push((previous, line_end));
                break;
            }
        }

        let consumed = line_end + CRLF_LEN;

        Ok(MemcacheRequest {
            buffer: buffer.split_to(consumed),
            command: MemcacheCommand::Get,
            keys,
            consumed,
            noreply: false,
            expiry: 0,
            flags: 0,
            value: (0, 0),
            cas: 0,
        })
    }

    fn parse_gets(buffer: &mut BytesMut) -> Result<MemcacheRequest, ParseError> {
        let mut request = MemcacheParser::parse_get(buffer)?;
        request.command = MemcacheCommand::Gets;
        Ok(request)
    }

    fn parse_set(buffer: &mut BytesMut) -> Result<MemcacheRequest, ParseError> {
        let buf: &[u8] = (*buffer).borrow();
        let mut single_byte = buf.windows(1);
        if let Some(cmd_end) = single_byte.position(|w| w == b" ") {
            // key
            let key_end = single_byte
                .position(|w| w == b" ")
                .ok_or(ParseError::Incomplete)?
                + cmd_end
                + 1;

            // flags
            let flags_end = single_byte
                .position(|w| w == b" ")
                .ok_or(ParseError::Incomplete)?
                + key_end
                + 1;
            let flags_str = std::str::from_utf8(&buf[(key_end + 1)..flags_end])
                .map_err(|_| ParseError::Invalid)?;
            let flags = flags_str.parse().map_err(|_| ParseError::Invalid)?;

            // expiry
            let expiry_end = single_byte
                .position(|w| w == b" ")
                .ok_or(ParseError::Incomplete)?
                + flags_end
                + 1;
            let expiry_str = std::str::from_utf8(&buf[(flags_end + 1)..expiry_end])
                .map_err(|_| ParseError::Invalid)?;
            let expiry = expiry_str.parse().map_err(|_| ParseError::Invalid)?;

            // now it gets tricky, we either have "[bytes] noreply\r\n" or "[bytes]\r\n"
            let mut double_byte = buf.windows(CRLF_LEN);
            let mut noreply = false;

            // get the position of the next space and first CRLF
            let next_space = single_byte
                .position(|w| w == b" ")
                .map(|v| v + expiry_end + 1);
            let first_crlf = double_byte
                .position(|w| w == CRLF)
                .ok_or(ParseError::Incomplete)?;

            let bytes_end = if let Some(next_space) = next_space {
                // if we have both, bytes_end is before the earlier of the two
                if next_space < first_crlf {
                    // validate that noreply isn't malformed
                    if &buf[(next_space + 1)..(first_crlf)] == NOREPLY {
                        noreply = true;
                        next_space
                    } else {
                        return Err(ParseError::Invalid);
                    }
                } else {
                    first_crlf
                }
            } else {
                first_crlf
            };

            // this checks for malformed requests where a CRLF is at an
            // unexpected part of the request
            if (expiry_end + 1) >= bytes_end {
                return Err(ParseError::Invalid);
            }

            if let Ok(Ok(bytes)) = std::str::from_utf8(&buffer[(expiry_end + 1)..bytes_end])
                .map(|v| v.parse::<usize>())
            {
                let consumed = first_crlf + CRLF_LEN + bytes + CRLF_LEN;
                if buf.len() >= consumed {
                    Ok(MemcacheRequest {
                        buffer: buffer.split_to(consumed),
                        command: MemcacheCommand::Set,
                        keys: vec![((cmd_end + 1), key_end)],
                        consumed,
                        noreply,
                        expiry,
                        flags,
                        value: ((first_crlf + CRLF_LEN), (first_crlf + CRLF_LEN + bytes)),
                        cas: 0,
                    })
                } else {
                    // the buffer doesn't yet have all the bytes for the value
                    Err(ParseError::Incomplete)
                }
            } else {
                // expiry couldn't be parsed
                Err(ParseError::Invalid)
            }
        } else {
            // no space (' ') in the buffer
            Err(ParseError::Incomplete)
        }
    }

    fn parse_add(buffer: &mut BytesMut) -> Result<MemcacheRequest, ParseError> {
        let mut request = MemcacheParser::parse_set(buffer)?;
        request.command = MemcacheCommand::Add;
        Ok(request)
    }

    fn parse_replace(buffer: &mut BytesMut) -> Result<MemcacheRequest, ParseError> {
        let mut request = MemcacheParser::parse_set(buffer)?;
        request.command = MemcacheCommand::Replace;
        Ok(request)
    }

    fn parse_cas(buffer: &mut BytesMut) -> Result<MemcacheRequest, ParseError> {
        let buf: &[u8] = (*buffer).borrow();
        let mut single_byte = buf.windows(1);
        // we already checked for this in the MemcacheParser::parse()
        let cmd_end = single_byte.position(|w| w == b" ").unwrap();
        let key_end = single_byte
            .position(|w| w == b" ")
            .ok_or(ParseError::Incomplete)?
            + cmd_end
            + 1;

        let flags_end = single_byte
            .position(|w| w == b" ")
            .ok_or(ParseError::Incomplete)?
            + key_end
            + 1;
        let flags_str =
            std::str::from_utf8(&buf[(key_end + 1)..flags_end]).map_err(|_| ParseError::Invalid)?;
        let flags = flags_str.parse().map_err(|_| ParseError::Invalid)?;

        let expiry_end = single_byte
            .position(|w| w == b" ")
            .ok_or(ParseError::Incomplete)?
            + flags_end
            + 1;
        let expiry_str = std::str::from_utf8(&buf[(flags_end + 1)..expiry_end])
            .map_err(|_| ParseError::Invalid)?;
        let expiry = expiry_str.parse().map_err(|_| ParseError::Invalid)?;

        let bytes_end = single_byte
            .position(|w| w == b" ")
            .ok_or(ParseError::Incomplete)?
            + expiry_end
            + 1;
        let bytes_str = std::str::from_utf8(&buf[(expiry_end + 1)..bytes_end])
            .map_err(|_| ParseError::Invalid)?;
        let bytes = bytes_str
            .parse::<usize>()
            .map_err(|_| ParseError::Invalid)?;

        // now it gets tricky, we either have "[bytes] noreply\r\n" or "[bytes]\r\n"
        let mut double_byte_windows = buf.windows(CRLF_LEN);
        let mut noreply = false;

        // get the position of the next space and first CRLF
        let next_space = single_byte
            .position(|w| w == b" ")
            .map(|v| v + expiry_end + 1);
        let first_crlf = double_byte_windows
            .position(|w| w == CRLF)
            .ok_or(ParseError::Incomplete)?;

        let cas_end = if let Some(next_space) = next_space {
            // if we have both, bytes_end is before the earlier of the two
            if next_space < first_crlf {
                // validate that noreply isn't malformed
                if &buf[(next_space + 1)..(first_crlf)] == NOREPLY {
                    noreply = true;
                    next_space
                } else {
                    return Err(ParseError::Invalid);
                }
            } else {
                first_crlf
            }
        } else {
            first_crlf
        };

        if (bytes_end + 1) >= cas_end {
            return Err(ParseError::Invalid);
        }

        if let Ok(Ok(cas)) =
            std::str::from_utf8(&buf[(bytes_end + 1)..cas_end]).map(|v| v.parse::<u64>())
        {
            let consumed = first_crlf + CRLF_LEN + bytes + CRLF_LEN;
            if buf.len() >= consumed {
                let buffer = buffer.split_to(consumed);
                Ok(MemcacheRequest {
                    buffer,
                    consumed,
                    command: MemcacheCommand::Cas,
                    keys: vec![((cmd_end + 1), key_end)],
                    flags,
                    expiry,
                    noreply,
                    cas,
                    value: ((first_crlf + CRLF_LEN), (first_crlf + CRLF_LEN + bytes)),
                })
            } else {
                // buffer doesn't have all the bytes for the value yet
                Err(ParseError::Incomplete)
            }
        } else {
            // could not parse the cas value
            Err(ParseError::Invalid)
        }
    }

    fn parse_delete(buffer: &mut BytesMut) -> Result<MemcacheRequest, ParseError> {
        let buf: &[u8] = (*buffer).borrow();
        let mut single_byte = buf.windows(1);
        // we already checked for this in the MemcacheParser::parse()
        let cmd_end = single_byte.position(|w| w == b" ").unwrap();

        let mut noreply = false;
        let mut double_byte = buf.windows(CRLF_LEN);
        // get the position of the next space and first CRLF
        let next_space = single_byte.position(|w| w == b" ").map(|v| v + cmd_end + 1);
        let first_crlf = double_byte
            .position(|w| w == CRLF)
            .ok_or(ParseError::Incomplete)?;

        let key_end = if let Some(next_space) = next_space {
            // if we have both, bytes_end is before the earlier of the two
            if next_space < first_crlf {
                // validate that noreply isn't malformed
                if &buf[(next_space + 1)..(first_crlf)] == NOREPLY {
                    noreply = true;
                    next_space
                } else {
                    return Err(ParseError::Invalid);
                }
            } else {
                first_crlf
            }
        } else {
            first_crlf
        };

        let consumed = if noreply {
            key_end + NOREPLY_LEN + CRLF_LEN
        } else {
            key_end + CRLF_LEN
        };

        let buffer = buffer.split_to(consumed);

        Ok(MemcacheRequest {
            buffer,
            command: MemcacheCommand::Delete,
            consumed,
            keys: vec![((cmd_end + 1), key_end)],
            noreply,
            cas: 0,
            expiry: 0,
            value: (0, 0),
            flags: 0,
        })
    }
}

pub enum MemcacheResponse {
    Deleted,
    End,
    Exists,
    Item { item: Item, cas: bool },
    NotFound,
    Stored,
    NotStored,
}

impl MemcacheResponse {
    pub fn serialize(self, buffer: &mut BytesMut) {
        match self {
            Self::Deleted => buffer.extend_from_slice(b"DELETED\r\n"),
            Self::End => buffer.extend_from_slice(b"END\r\n"),
            Self::Exists => buffer.extend_from_slice(b"EXISTS\r\n"),
            Self::Item { item, cas } => {
                buffer.extend_from_slice(b"VALUE ");
                buffer.extend_from_slice(item.key());
                let f = item.optional().unwrap();
                let flags: u32 = u32::from_be_bytes([f[0], f[1], f[2], f[3]]);
                if cas {
                    buffer.extend_from_slice(
                        format!(" {} {} {}", flags, item.value().len(), item.cas()).as_bytes(),
                    );
                } else {
                    buffer
                        .extend_from_slice(format!(" {} {}", flags, item.value().len()).as_bytes());
                }
                buffer.extend_from_slice(CRLF);
                buffer.extend_from_slice(item.value());
                buffer.extend_from_slice(CRLF);
            }
            Self::NotFound => buffer.extend_from_slice(b"NOT_FOUND\r\n"),
            Self::NotStored => buffer.extend_from_slice(b"NOT_STORED\r\n"),
            Self::Stored => buffer.extend_from_slice(b"STORED\r\n"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        for mut buffer in buffers.iter().map(|v| BytesMut::from(&v[..])) {
            println!("parse: {:?}", buffer);
            assert_eq!(
                MemcacheParser::parse(&mut buffer),
                Err(ParseError::Incomplete)
            );
        }
    }

    #[test]
    fn parse_get() {
        for key in keys() {
            let mut buffer = BytesMut::new();
            buffer.extend_from_slice(b"get ");
            buffer.extend_from_slice(key);
            buffer.extend_from_slice(b"\r\n");
            let parsed = MemcacheParser::parse(&mut buffer);
            assert!(parsed.is_ok());
            if let Ok(request) = parsed {
                assert_eq!(request.keys(), vec![key]);
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
                assert_eq!(request.keys(), vec![key]);
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
                        assert_eq!(request.keys(), vec![key]);
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
