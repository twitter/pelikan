// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use super::super::*;
use crate::*;

use std::convert::TryFrom;

impl Parse for MemcacheRequest {
    fn parse(buffer: &[u8]) -> Result<ParseOk<Self>, ParseError> {
        match parse_command(buffer)? {
            MemcacheCommand::Get => parse_get(buffer),
            MemcacheCommand::Gets => parse_gets(buffer),
            MemcacheCommand::Set => parse_set(buffer),
            MemcacheCommand::Add => parse_add(buffer),
            MemcacheCommand::Replace => parse_replace(buffer),
            MemcacheCommand::Cas => parse_cas(buffer),
            MemcacheCommand::Delete => parse_delete(buffer),
        }
    }
}

fn parse_command(buffer: &[u8]) -> Result<MemcacheCommand, ParseError> {
    let command;
    {
        // check if we got a CRLF
        let mut double_byte = buffer.windows(CRLF.len());
        if let Some(_line_end) = double_byte.position(|w| w == CRLF.as_bytes()) {
            // single-byte windowing to find spaces
            let mut single_byte = buffer.windows(1);
            if let Some(cmd_end) = single_byte.position(|w| w == b" ") {
                command = MemcacheCommand::try_from(&buffer[0..cmd_end])?;
            } else {
                return Err(ParseError::Incomplete);
            }
        } else {
            return Err(ParseError::Incomplete);
        }
    }
    Ok(command)
}

#[allow(clippy::unnecessary_wraps)]
fn parse_get(buffer: &[u8]) -> Result<ParseOk<MemcacheRequest>, ParseError> {
    let mut double_byte = buffer.windows(CRLF.len());
    let line_end = double_byte.position(|w| w == CRLF.as_bytes()).unwrap();

    let mut single_byte = buffer.windows(1);
    // we already checked for this in the MemcacheParser::parse()
    let cmd_end = single_byte.position(|w| w == b" ").unwrap();
    let mut previous = cmd_end + 1;
    let mut keys = Vec::new();

    // command may have multiple keys, we need to loop until we hit
    // a CRLF
    loop {
        if let Some(key_end) = single_byte.position(|w| w == b" ") {
            if key_end < line_end {
                keys.push(buffer[previous..key_end].to_vec().into_boxed_slice());
                previous = key_end + 1;
            } else {
                keys.push(buffer[previous..line_end].to_vec().into_boxed_slice());
                break;
            }
        } else {
            keys.push(buffer[previous..line_end].to_vec().into_boxed_slice());
            break;
        }
    }

    let consumed = line_end + CRLF.len();

    let message = MemcacheRequest::Get {
        keys: keys.into_boxed_slice(),
    };
    Ok(ParseOk { message, consumed })
}

fn parse_gets(buffer: &[u8]) -> Result<ParseOk<MemcacheRequest>, ParseError> {
    let request = parse_get(buffer)?;
    let consumed = request.consumed();
    let message = if let MemcacheRequest::Get { keys } = request.into_inner() {
        MemcacheRequest::Gets { keys }
    } else {
        unreachable!()
    };

    Ok(ParseOk { message, consumed })
}

fn parse_set(buffer: &[u8]) -> Result<ParseOk<MemcacheRequest>, ParseError> {
    let mut single_byte = buffer.windows(1);
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
        let flags_str = std::str::from_utf8(&buffer[(key_end + 1)..flags_end])
            .map_err(|_| ParseError::Invalid)?;
        let flags = flags_str.parse().map_err(|_| ParseError::Invalid)?;

        // expiry
        let expiry_end = single_byte
            .position(|w| w == b" ")
            .ok_or(ParseError::Incomplete)?
            + flags_end
            + 1;
        let expiry_str = std::str::from_utf8(&buffer[(flags_end + 1)..expiry_end])
            .map_err(|_| ParseError::Invalid)?;
        let expiry = expiry_str.parse().map_err(|_| ParseError::Invalid)?;

        // now it gets tricky, we either have "[bytes] noreply\r\n" or "[bytes]\r\n"
        let mut double_byte = buffer.windows(CRLF.len());
        let mut noreply = false;

        // get the position of the next space and first CRLF
        let next_space = single_byte
            .position(|w| w == b" ")
            .map(|v| v + expiry_end + 1);
        let first_crlf = double_byte
            .position(|w| w == CRLF.as_bytes())
            .ok_or(ParseError::Incomplete)?;

        let bytes_end = if let Some(next_space) = next_space {
            // if we have both, bytes_end is before the earlier of the two
            if next_space < first_crlf {
                // validate that noreply isn't malformed
                if &buffer[(next_space + 1)..(first_crlf)] == NOREPLY.as_bytes() {
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
            std::str::from_utf8(&buffer[(expiry_end + 1)..bytes_end]).map(|v| v.parse::<usize>())
        {
            let consumed = first_crlf + CRLF.len() + bytes + CRLF.len();
            if buffer.len() >= consumed {
                let key = buffer[(cmd_end + 1)..key_end].to_vec().into_boxed_slice();
                let value = buffer[(first_crlf + CRLF.len())..(first_crlf + CRLF.len() + bytes)]
                    .to_vec()
                    .into_boxed_slice();

                let entry = MemcacheEntry {
                    key,
                    value,
                    cas: None,
                    expiry,
                    flags,
                };
                Ok(ParseOk {
                    message: MemcacheRequest::Set { entry, noreply },
                    consumed,
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

fn parse_add(buffer: &[u8]) -> Result<ParseOk<MemcacheRequest>, ParseError> {
    let request = parse_set(buffer)?;
    let consumed = request.consumed();

    let message = if let MemcacheRequest::Set { entry, noreply } = request.into_inner() {
        MemcacheRequest::Add { entry, noreply }
    } else {
        unreachable!()
    };

    Ok(ParseOk { message, consumed })
}

fn parse_replace(buffer: &[u8]) -> Result<ParseOk<MemcacheRequest>, ParseError> {
    let request = parse_set(buffer)?;
    let consumed = request.consumed();

    let message = if let MemcacheRequest::Set { entry, noreply } = request.into_inner() {
        MemcacheRequest::Replace { entry, noreply }
    } else {
        unreachable!()
    };

    Ok(ParseOk { message, consumed })
}

fn parse_cas(buffer: &[u8]) -> Result<ParseOk<MemcacheRequest>, ParseError> {
    let mut single_byte = buffer.windows(1);
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
        std::str::from_utf8(&buffer[(key_end + 1)..flags_end]).map_err(|_| ParseError::Invalid)?;
    let flags = flags_str.parse().map_err(|_| ParseError::Invalid)?;

    let expiry_end = single_byte
        .position(|w| w == b" ")
        .ok_or(ParseError::Incomplete)?
        + flags_end
        + 1;
    let expiry_str = std::str::from_utf8(&buffer[(flags_end + 1)..expiry_end])
        .map_err(|_| ParseError::Invalid)?;
    let expiry = expiry_str.parse().map_err(|_| ParseError::Invalid)?;

    let bytes_end = single_byte
        .position(|w| w == b" ")
        .ok_or(ParseError::Incomplete)?
        + expiry_end
        + 1;
    let bytes_str = std::str::from_utf8(&buffer[(expiry_end + 1)..bytes_end])
        .map_err(|_| ParseError::Invalid)?;
    let bytes = bytes_str
        .parse::<usize>()
        .map_err(|_| ParseError::Invalid)?;

    // now it gets tricky, we either have "[bytes] noreply\r\n" or "[bytes]\r\n"
    let mut double_byte_windows = buffer.windows(CRLF.len());
    let mut noreply = false;

    // get the position of the next space and first CRLF
    let next_space = single_byte
        .position(|w| w == b" ")
        .map(|v| v + expiry_end + 1);
    let first_crlf = double_byte_windows
        .position(|w| w == CRLF.as_bytes())
        .ok_or(ParseError::Incomplete)?;

    let cas_end = if let Some(next_space) = next_space {
        // if we have both, bytes_end is before the earlier of the two
        if next_space < first_crlf {
            // validate that noreply isn't malformed
            if &buffer[(next_space + 1)..(first_crlf)] == NOREPLY.as_bytes() {
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
        std::str::from_utf8(&buffer[(bytes_end + 1)..cas_end]).map(|v| v.parse::<u64>())
    {
        let consumed = first_crlf + CRLF.len() + bytes + CRLF.len();
        if buffer.len() >= consumed {
            // let buffer = buffer.split_to(consumed);
            let key = buffer[(cmd_end + 1)..key_end].to_vec().into_boxed_slice();
            let value = buffer[(first_crlf + CRLF.len())..(first_crlf + CRLF.len() + bytes)]
                .to_vec()
                .into_boxed_slice();

            let entry = MemcacheEntry {
                key,
                value,
                cas: Some(cas),
                flags,
                expiry,
            };
            Ok(ParseOk {
                message: MemcacheRequest::Cas { entry, noreply },
                consumed,
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

fn parse_delete(buffer: &[u8]) -> Result<ParseOk<MemcacheRequest>, ParseError> {
    let mut single_byte = buffer.windows(1);
    // we already checked for this in the MemcacheParser::parse()
    let cmd_end = single_byte.position(|w| w == b" ").unwrap();

    let mut noreply = false;
    let mut double_byte = buffer.windows(CRLF.len());
    // get the position of the next space and first CRLF
    let next_space = single_byte.position(|w| w == b" ").map(|v| v + cmd_end + 1);
    let first_crlf = double_byte
        .position(|w| w == CRLF.as_bytes())
        .ok_or(ParseError::Incomplete)?;

    let key_end = if let Some(next_space) = next_space {
        // if we have both, bytes_end is before the earlier of the two
        if next_space < first_crlf {
            // validate that noreply isn't malformed
            if &buffer[(next_space + 1)..(first_crlf)] == NOREPLY.as_bytes() {
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
        key_end + NOREPLY.len() + CRLF.len()
    } else {
        key_end + CRLF.len()
    };

    let request = MemcacheRequest::Delete {
        key: buffer[(cmd_end + 1)..key_end].to_vec().into_boxed_slice(),
        noreply,
    };

    Ok(ParseOk {
        message: request,
        consumed,
    })
}
