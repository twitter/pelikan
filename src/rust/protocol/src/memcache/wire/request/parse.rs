// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use super::super::*;
use crate::*;

use config::TimeType;

use std::convert::TryFrom;

const MAX_COMMAND_LEN: usize = 16;
const MAX_KEY_LEN: usize = 250;
const MAX_BATCH_SIZE: usize = 1024;

const DEFAULT_MAX_VALUE_SIZE: usize = usize::MAX / 2;

#[derive(Copy, Clone)]
pub struct MemcacheRequestParser {
    max_value_size: usize,
    time_type: TimeType,
}

impl MemcacheRequestParser {
    pub fn new(max_value_size: usize, time_type: TimeType) -> Self {
        Self {
            max_value_size,
            time_type,
        }
    }
}

impl Default for MemcacheRequestParser {
    fn default() -> Self {
        Self {
            max_value_size: DEFAULT_MAX_VALUE_SIZE,
            time_type: config::time::DEFAULT_TIME_TYPE,
        }
    }
}

impl Parse<MemcacheRequest> for MemcacheRequestParser {
    fn parse(&self, buffer: &[u8]) -> Result<ParseOk<MemcacheRequest>, ParseError> {
        match parse_command(buffer)? {
            MemcacheCommand::Get => parse_get(buffer),
            MemcacheCommand::Gets => parse_gets(buffer),
            MemcacheCommand::Set => parse_set(buffer, false, self.max_value_size, self.time_type),
            MemcacheCommand::Add => parse_add(buffer, self.max_value_size, self.time_type),
            MemcacheCommand::Replace => parse_replace(buffer, self.max_value_size, self.time_type),
            MemcacheCommand::Cas => parse_set(buffer, true, self.max_value_size, self.time_type),
            MemcacheCommand::Delete => parse_delete(buffer),
            MemcacheCommand::Quit => {
                // TODO(bmartin): in-band control commands need to be handled
                // differently, this is a quick hack to emulate the 'quit'
                // command
                Err(ParseError::Invalid)
            }
        }
    }
}

struct ParseState<'a> {
    buffer: &'a [u8],
    position: usize,
}

#[derive(PartialEq)]
enum Sequence {
    Space,
    Crlf,
    SpaceCrlf,
}

impl Sequence {
    pub fn len(&self) -> usize {
        match self {
            Self::Space => 1,
            Self::Crlf => 2,
            Self::SpaceCrlf => 3,
        }
    }
}

impl<'a> ParseState<'a> {
    fn new(buffer: &'a [u8]) -> Self {
        Self {
            buffer,
            position: 0,
        }
    }

    fn position(&self) -> usize {
        self.position
    }

    fn next_sequence(&mut self) -> Option<(Sequence, usize)> {
        for i in self.position..self.buffer.len() {
            match self.buffer[i] {
                b' ' => {
                    if self.buffer.len() > i + 2
                        && self.buffer[i + 1] == b'\r'
                        && self.buffer[i + 2] == b'\n'
                    {
                        let s = Sequence::SpaceCrlf;
                        self.position += s.len();
                        return Some((s, i));
                    } else {
                        let s = Sequence::Space;
                        self.position += s.len();
                        return Some((s, i));
                    }
                }
                b'\r' => {
                    if self.buffer.len() > i + 1 && self.buffer[i + 1] == b'\n' {
                        let s = Sequence::Crlf;
                        self.position += s.len();
                        return Some((s, i));
                    } else {
                        self.position += 1;
                    }
                }
                b'\0' => {
                    return None;
                }
                _ => {
                    self.position += 1;
                }
            }
        }
        None
    }
}

#[allow(clippy::unnecessary_unwrap)]
fn parse_command(buffer: &[u8]) -> Result<MemcacheCommand, ParseError> {
    let mut parse_state = ParseState::new(buffer);
    if let Some((_, position)) = parse_state.next_sequence() {
        Ok(MemcacheCommand::try_from(&buffer[0..position])?)
    } else if buffer.len() > MAX_COMMAND_LEN {
        Err(ParseError::Invalid)
    } else {
        Err(ParseError::Incomplete)
    }
}

#[allow(clippy::unnecessary_wraps)]
fn parse_get(buffer: &[u8]) -> Result<ParseOk<MemcacheRequest>, ParseError> {
    let mut parse_state = ParseState::new(buffer);

    // this was already checked for when determining the command
    let (whitespace, cmd_end) = parse_state.next_sequence().unwrap();

    match whitespace {
        Sequence::Crlf | Sequence::SpaceCrlf => {
            return Err(ParseError::Invalid);
        }
        _ => {}
    }

    let mut previous = cmd_end + 1;
    let mut keys = Vec::new();

    // command may have multiple keys, we need to loop until we hit
    // a CRLF
    while let Some((sequence, key_end)) = parse_state.next_sequence() {
        match sequence {
            Sequence::Space => {
                if key_end <= previous || key_end > previous + MAX_KEY_LEN {
                    return Err(ParseError::Invalid);
                } else {
                    keys.push(buffer[previous..key_end].to_vec().into_boxed_slice());

                    previous = key_end + whitespace.len();

                    if keys.len() >= MAX_BATCH_SIZE {
                        return Err(ParseError::Invalid);
                    }
                }
            }
            Sequence::Crlf | Sequence::SpaceCrlf => {
                if key_end > previous && key_end <= previous + MAX_KEY_LEN {
                    keys.push(buffer[previous..key_end].to_vec().into_boxed_slice());

                    let consumed = key_end + whitespace.len();

                    if keys.is_empty() {
                        return Err(ParseError::Invalid);
                    } else {
                        let message = MemcacheRequest::Get {
                            keys: keys.into_boxed_slice(),
                        };
                        return Ok(ParseOk { message, consumed });
                    }
                } else {
                    return Err(ParseError::Invalid);
                }
            }
        }
    }
    if buffer.len() > cmd_end + MAX_KEY_LEN {
        Err(ParseError::Invalid)
    } else {
        Err(ParseError::Incomplete)
    }
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

fn parse_set(
    buffer: &[u8],
    cas: bool,
    max_value_size: usize,
    time_type: TimeType,
) -> Result<ParseOk<MemcacheRequest>, ParseError> {
    let mut parse_state = ParseState::new(buffer);

    // this was already checked for when determining the command
    let (whitespace, cmd_end) = parse_state.next_sequence().unwrap();

    if whitespace != Sequence::Space {
        return Err(ParseError::Invalid);
    }

    // key
    let (whitespace, key_end) = parse_state.next_sequence().ok_or(ParseError::Incomplete)?;
    if whitespace != Sequence::Space
        || key_end <= cmd_end + 1
        || key_end - (cmd_end + 1) > MAX_KEY_LEN
    {
        return Err(ParseError::Invalid);
    }

    // flags
    let (whitespace, flags_end) = parse_state.next_sequence().ok_or(ParseError::Incomplete)?;
    if whitespace != Sequence::Space {
        return Err(ParseError::Invalid);
    }
    let flags_str =
        std::str::from_utf8(&buffer[(key_end + 1)..flags_end]).map_err(|_| ParseError::Invalid)?;
    let flags = flags_str.parse().map_err(|_| ParseError::Invalid)?;

    // expiry
    let (whitespace, expiry_end) = parse_state.next_sequence().ok_or(ParseError::Incomplete)?;
    if whitespace != Sequence::Space {
        return Err(ParseError::Invalid);
    }
    let expiry_str = std::str::from_utf8(&buffer[(flags_end + 1)..expiry_end])
        .map_err(|_| ParseError::Invalid)?;
    let expiry: u32 = expiry_str.parse().map_err(|_| ParseError::Invalid)?;
    let ttl = if time_type == TimeType::Unix
        || (time_type == TimeType::Memcache && expiry >= 60 * 60 * 24 * 30)
    {
        Some(expiry.saturating_sub(rustcommon_time::recent_unix()))
    } else if expiry == 0 {
        None
    } else {
        Some(expiry)
    };

    let mut noreply = false;

    let (whitespace, bytes_end) = parse_state.next_sequence().ok_or(ParseError::Incomplete)?;
    let cas_end = if cas {
        if whitespace != Sequence::Space {
            return Err(ParseError::Invalid);
        }
        let (whitespace, cas_end) = parse_state.next_sequence().ok_or(ParseError::Incomplete)?;
        if whitespace == Sequence::Space {
            let (_whitespace, noreply_end) =
                parse_state.next_sequence().ok_or(ParseError::Incomplete)?;
            if &buffer[(noreply_end - NOREPLY.len())..noreply_end] == NOREPLY.as_bytes() {
                noreply = true;
            } else {
                return Err(ParseError::Invalid);
            }
        }
        Some(cas_end)
    } else if whitespace == Sequence::Space {
        let (_whitespace, noreply_end) =
            parse_state.next_sequence().ok_or(ParseError::Incomplete)?;
        if &buffer[(noreply_end - NOREPLY.len())..noreply_end] == NOREPLY.as_bytes() {
            noreply = true;
        } else {
            return Err(ParseError::Invalid);
        }
        None
    } else {
        None
    };

    let bytes_str = std::str::from_utf8(&buffer[(expiry_end + 1)..bytes_end])
        .map_err(|_| ParseError::Invalid)?;
    let bytes = bytes_str
        .parse::<usize>()
        .map_err(|_| ParseError::Invalid)?;

    if bytes > max_value_size {
        return Err(ParseError::Invalid);
    }

    let cas = if let Some(cas_end) = cas_end {
        if (bytes_end + 1) >= cas_end {
            return Err(ParseError::Invalid);
        }
        let cas_str = std::str::from_utf8(&buffer[(bytes_end + 1)..cas_end])
            .map_err(|_| ParseError::Invalid)?;
        Some(cas_str.parse::<u64>().map_err(|_| ParseError::Invalid)?)
    } else {
        None
    };

    let consumed = parse_state.position() + bytes + CRLF.len();
    if buffer.len() >= consumed {
        let key = buffer[(cmd_end + 1)..key_end].to_vec().into_boxed_slice();
        let value = buffer[parse_state.position()..(parse_state.position() + bytes)]
            .to_vec()
            .into_boxed_slice();

        let entry = MemcacheEntry {
            key,
            value,
            ttl,
            flags,
            cas,
        };
        if cas.is_some() {
            Ok(ParseOk {
                message: MemcacheRequest::Cas { entry, noreply },
                consumed,
            })
        } else {
            Ok(ParseOk {
                message: MemcacheRequest::Set { entry, noreply },
                consumed,
            })
        }
    } else {
        // the buffer doesn't yet have all the bytes for the value
        Err(ParseError::Incomplete)
    }
}

fn parse_add(
    buffer: &[u8],
    max_value_size: usize,
    time_type: TimeType,
) -> Result<ParseOk<MemcacheRequest>, ParseError> {
    let request = parse_set(buffer, false, max_value_size, time_type)?;
    let consumed = request.consumed();

    let message = if let MemcacheRequest::Set { entry, noreply } = request.into_inner() {
        MemcacheRequest::Add { entry, noreply }
    } else {
        unreachable!()
    };

    Ok(ParseOk { message, consumed })
}

fn parse_replace(
    buffer: &[u8],
    max_value_size: usize,
    time_type: TimeType,
) -> Result<ParseOk<MemcacheRequest>, ParseError> {
    let request = parse_set(buffer, false, max_value_size, time_type)?;
    let consumed = request.consumed();

    let message = if let MemcacheRequest::Set { entry, noreply } = request.into_inner() {
        MemcacheRequest::Replace { entry, noreply }
    } else {
        unreachable!()
    };

    Ok(ParseOk { message, consumed })
}

fn parse_delete(buffer: &[u8]) -> Result<ParseOk<MemcacheRequest>, ParseError> {
    let mut parse_state = ParseState::new(buffer);

    // this was already checked for when determining the command
    let (whitespace, cmd_end) = parse_state.next_sequence().unwrap();

    if whitespace != Sequence::Space {
        return Err(ParseError::Invalid);
    }

    let mut noreply = false;

    let (whitespace, key_end) = parse_state.next_sequence().ok_or(ParseError::Incomplete)?;
    if whitespace == Sequence::Space {
        let (_whitespace, noreply_end) =
            parse_state.next_sequence().ok_or(ParseError::Incomplete)?;
        if &buffer[(noreply_end - NOREPLY.len())..noreply_end] == NOREPLY.as_bytes() {
            noreply = true;
        } else {
            return Err(ParseError::Invalid);
        }
    }

    let consumed = parse_state.position();

    if key_end <= (cmd_end + 1) {
        return Err(ParseError::Invalid);
    }

    if key_end - (cmd_end + 1) > MAX_KEY_LEN {
        return Err(ParseError::Invalid);
    }

    let request = MemcacheRequest::Delete {
        key: buffer[(cmd_end + 1)..key_end].to_vec().into_boxed_slice(),
        noreply,
    };

    Ok(ParseOk {
        message: request,
        consumed,
    })
}
