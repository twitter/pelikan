// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Memcache data protocol parsing

use crate::protocol::data::request::Index;
use bytes::BytesMut;
use std::borrow::Borrow;

use super::*;

// TODO(bmartin): this should be lifted out into a common crate and shared
// between different protocols
// TODO(bmartin): add response parsing as well so this can be shared between
// both client and server implementations
pub trait Parser {
    type Request: Request;

    fn parse(buffer: &mut BytesMut) -> Result<Self::Request, ParseError>;
}

/// A struct for parsing memcache data protocol
pub struct MemcacheParser;

impl Parser for MemcacheParser {
    type Request = MemcacheRequest;

    fn parse<'a>(buffer: &mut BytesMut) -> Result<Self::Request, ParseError> {
        let command;
        {
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
                    keys.push(Index { start: previous, end: key_end });
                    previous = key_end + 1;
                } else {
                    keys.push(Index { start: previous, end: line_end });
                    break;
                }
            } else {
                keys.push(Index { start: previous, end: line_end });
                break;
            }
        }

        let consumed = line_end + CRLF_LEN;
        let buffer = buffer.split_to(consumed);

        Ok(MemcacheRequest {
            buffer,
            command: MemcacheCommand::Get,
            keys,
            consumed,
            noreply: false,
            expiry: 0,
            flags: 0,
            value: Index { start: 0, end: 0 },
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

            if let Ok(Ok(bytes)) =
                std::str::from_utf8(&buf[(expiry_end + 1)..bytes_end]).map(|v| v.parse::<usize>())
            {
                let consumed = first_crlf + CRLF_LEN + bytes + CRLF_LEN;
                if buf.len() >= consumed {
                    let buffer = buffer.split_to(consumed);
                    Ok(MemcacheRequest {
                        buffer,
                        command: MemcacheCommand::Set,
                        keys: vec![Index { start: (cmd_end + 1), end: key_end }],
                        consumed,
                        noreply,
                        expiry,
                        flags,
                        value: Index { start: (first_crlf + CRLF_LEN), end: (first_crlf + CRLF_LEN + bytes) },
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
                    keys: vec![Index { start: (cmd_end + 1), end: key_end }],
                    flags,
                    expiry,
                    noreply,
                    cas,
                    value: Index { start: (first_crlf + CRLF_LEN), end: (first_crlf + CRLF_LEN + bytes) },
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
            keys: vec![Index { start: (cmd_end + 1), end: key_end }],
            noreply,
            cas: 0,
            expiry: 0,
            value: Index { start: 0, end: 0 },
            flags: 0,
        })
    }
}
