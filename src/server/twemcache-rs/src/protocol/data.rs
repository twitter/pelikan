// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::protocol::{CRLF, CRLF_LEN};
use crate::Config;
use bytes::BytesMut;
use config::TimeType;
use metrics::*;
use rustcommon_time::CoarseDuration;
use segcache::{SegCache, SegCacheError};
use std::borrow::Borrow;
use std::sync::Arc;
use std::time::SystemTime;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Verb {
    Get,
    Gets,
    Set,
    Cas,
    Add,
    Replace,
    Delete,
}

// TODO(bmartin): evaluate if we take any performance hit from using a request
// trait here instead. Trying to avoid a Box<>. Another option might be a
// unified type, but that moves some of the complexity back into the request
// handling.
#[derive(PartialEq, Eq, Debug)]
pub enum Request {
    Get(GetRequest),
    Gets(GetsRequest),
    Set(SetRequest),
    Cas(CasRequest),
    Add(AddRequest),
    Replace(ReplaceRequest),
    Delete(DeleteRequest),
}

impl Request {
    pub fn process(
        self,
        config: &Arc<Config>,
        write_buffer: &mut BytesMut,
        data: &mut SegCache,
    ) {
        match self {
            Self::Get(r) => process_get(r, write_buffer, data),
            Self::Gets(r) => process_gets(r, write_buffer, data),
            Self::Set(r) => process_set(config, r, write_buffer, data),
            Self::Cas(r) => process_cas(config, r, write_buffer, data),
            Self::Add(r) => process_add(config, r, write_buffer, data),
            Self::Replace(r) => process_replace(config, r, write_buffer, data),
            Self::Delete(r) => process_delete(r, write_buffer, data),
        }
    }
}

#[derive(PartialEq, Eq, Debug)]
pub enum ParseError {
    Incomplete,
    Invalid,
    UnknownCommand,
}

#[derive(PartialEq, Eq, Debug)]
pub struct GetRequest {
    data: BytesMut,
    key_indices: Vec<(usize, usize)>,
}

impl GetRequest {
    pub fn keys(&self) -> Vec<&[u8]> {
        let data: &[u8] = self.data.borrow();
        let mut keys = Vec::new();
        for key_index in &self.key_indices {
            keys.push(&data[key_index.0..key_index.1])
        }
        keys
    }
}

pub fn process_get(
    request: GetRequest,
    write_buffer: &mut BytesMut,
    data: &mut SegCache,
) {
    let mut found = 0;
    increment_counter!(&Stat::Get);
    for key in request.keys() {
        if let Some(item) = data.get(key) {
            write_buffer.extend_from_slice(b"VALUE ");
            write_buffer.extend_from_slice(key);
            let f = item.optional().unwrap();
            let flags: u32 = u32::from_be_bytes([f[0], f[1], f[2], f[3]]);
            write_buffer.extend_from_slice(format!(" {} {}", flags, item.value().len()).as_bytes());
            write_buffer.extend_from_slice(CRLF);
            write_buffer.extend_from_slice(item.value());
            write_buffer.extend_from_slice(CRLF);
            found += 1;
        }
    }
    let total = request.keys().len() as u64;
    increment_counter_by!(&Stat::GetKey, total);
    increment_counter_by!(&Stat::GetKeyHit, found);
    increment_counter_by!(&Stat::GetKeyMiss, total - found);

    debug!(
        "get request processed, {} out of {} keys found",
        found, total
    );
    write_buffer.extend_from_slice(b"END\r\n");
}

#[derive(PartialEq, Eq, Debug)]
pub struct GetsRequest {
    data: BytesMut,
    key_indices: Vec<(usize, usize)>,
}

impl GetsRequest {
    pub fn keys(&self) -> Vec<&[u8]> {
        let data: &[u8] = self.data.borrow();
        let mut keys = Vec::new();
        for key_index in &self.key_indices {
            keys.push(&data[key_index.0..key_index.1])
        }
        keys
    }
}

pub fn process_gets(
    request: GetsRequest,
    write_buffer: &mut BytesMut,
    data: &mut SegCache,
) {
    let mut found = 0;
    increment_counter!(&Stat::Gets);
    for key in request.keys() {
        if let Some(item) = data.get(key) {
            write_buffer.extend_from_slice(b"VALUE ");
            write_buffer.extend_from_slice(key);
            let f = item.optional().unwrap();
            let flags: u32 = u32::from_be_bytes([f[0], f[1], f[2], f[3]]);
            write_buffer.extend_from_slice(
                format!(" {} {} {}", flags, item.value().len(), item.cas()).as_bytes(),
            );
            write_buffer.extend_from_slice(CRLF);
            write_buffer.extend_from_slice(item.value());
            write_buffer.extend_from_slice(CRLF);
            found += 1;
        }
    }
    let total = request.keys().len() as u64;
    increment_counter_by!(&Stat::GetsKey, total);
    increment_counter_by!(&Stat::GetsKeyHit, found);
    increment_counter_by!(&Stat::GetsKeyMiss, total - found);
    debug!(
        "get request processed, {} out of {} keys found",
        found, total
    );
    write_buffer.extend_from_slice(b"END\r\n");
}

#[derive(PartialEq, Eq, Debug)]
pub struct DeleteRequest {
    data: BytesMut,
    key_index: (usize, usize),
    noreply: bool,
}

impl DeleteRequest {
    pub fn key(&self) -> &[u8] {
        let data: &[u8] = self.data.borrow();
        &data[self.key_index.0..self.key_index.1]
    }

    pub fn noreply(&self) -> bool {
        self.noreply
    }
}

pub fn process_delete(
    request: DeleteRequest,
    write_buffer: &mut BytesMut,
    data: &mut SegCache,
) {
    increment_counter!(&Stat::Delete);
    let reply = !request.noreply();
    #[allow(clippy::collapsible_if)]
    if data.delete(request.key()) {
        increment_counter!(&Stat::DeleteDeleted);
        if reply {
            write_buffer.extend_from_slice(b"DELETED\r\n");
        }
    } else {
        increment_counter!(&Stat::DeleteNotfound);
        if reply {
            write_buffer.extend_from_slice(b"NOT_FOUND\r\n");
        }
    }
}

#[derive(PartialEq, Eq, Debug)]
pub struct SetRequest {
    data: BytesMut,
    key_index: (usize, usize),
    expiry: u32,
    flags: u32,
    noreply: bool,
    value_index: (usize, usize),
}

impl SetRequest {
    pub fn key(&self) -> &[u8] {
        let data: &[u8] = self.data.borrow();
        &data[self.key_index.0..self.key_index.1]
    }

    pub fn value(&self) -> &[u8] {
        let data: &[u8] = self.data.borrow();
        &data[self.value_index.0..self.value_index.1]
    }

    pub fn flags(&self) -> u32 {
        self.flags
    }

    pub fn noreply(&self) -> bool {
        self.noreply
    }

    pub fn expiry(&self) -> u32 {
        self.expiry
    }
}

pub fn process_set(
    config: &Arc<Config>,
    request: SetRequest,
    write_buffer: &mut BytesMut,
    data: &mut SegCache,
) {
    increment_counter!(&Stat::Set);
    let reply = !request.noreply();

    // convert the expiry to a delta TTL
    let ttl: u32 = match config.time().time_type() {
        TimeType::Unix => {
            let epoch = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs() as u32;
            request.expiry().wrapping_sub(epoch)
        }
        TimeType::Delta => request.expiry(),
        TimeType::Memcache => {
            if request.expiry() == 0 {
                0
            } else if request.expiry() < 60 * 60 * 24 * 30 {
                request.expiry()
            } else {
                let epoch = SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as u32;
                request.expiry().wrapping_sub(epoch)
            }
        }
    };
    #[allow(clippy::collapsible_if)]
    match data.insert(
        request.key(),
        request.value(),
        Some(&request.flags().to_be_bytes()),
        CoarseDuration::from_secs(ttl),
    ) {
        Ok(_) => {
            increment_counter!(&Stat::SetStored);
            if reply {
                write_buffer.extend_from_slice(b"STORED\r\n");
            }
        }
        Err(_) => {
            increment_counter!(&Stat::SetNotstored);
            if reply {
                write_buffer.extend_from_slice(b"NOT_STORED\r\n");
            }
        }
    }
}

#[derive(PartialEq, Eq, Debug)]
pub struct CasRequest {
    data: BytesMut,
    key_index: (usize, usize),
    expiry: u32,
    flags: u32,
    cas: u64,
    noreply: bool,
    value_index: (usize, usize),
}

impl CasRequest {
    pub fn key(&self) -> &[u8] {
        let data: &[u8] = self.data.borrow();
        &data[self.key_index.0..self.key_index.1]
    }

    pub fn value(&self) -> &[u8] {
        let data: &[u8] = self.data.borrow();
        &data[self.value_index.0..self.value_index.1]
    }

    pub fn flags(&self) -> u32 {
        self.flags
    }

    pub fn noreply(&self) -> bool {
        self.noreply
    }

    pub fn expiry(&self) -> u32 {
        self.expiry
    }

    pub fn cas(&self) -> u64 {
        self.cas
    }
}

pub fn process_cas(
    config: &Arc<Config>,
    request: CasRequest,
    write_buffer: &mut BytesMut,
    data: &mut SegCache,
) {
    increment_counter!(&Stat::Cas);
    let reply = !request.noreply();
    // convert the expiry to a delta TTL
    let ttl: u32 = match config.time().time_type() {
        TimeType::Unix => {
            let epoch = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs() as u32;
            request.expiry().wrapping_sub(epoch)
        }
        TimeType::Delta => request.expiry(),
        TimeType::Memcache => {
            if request.expiry() == 0 {
                0
            } else if request.expiry() < 60 * 60 * 24 * 30 {
                request.expiry()
            } else {
                let epoch = SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as u32;
                request.expiry().wrapping_sub(epoch)
            }
        }
    };
    match data.cas(
        request.key(),
        request.value(),
        Some(&request.flags().to_be_bytes()),
        CoarseDuration::from_secs(ttl),
        request.cas() as u32,
    ) {
        Ok(_) => {
            increment_counter!(&Stat::CasStored);
            if reply {
                write_buffer.extend_from_slice(b"STORED\r\n");
            }
        }
        Err(SegCacheError::NotFound) => {
            increment_counter!(&Stat::CasNotfound);
            if reply {
                write_buffer.extend_from_slice(b"NOT_FOUND\r\n");
            }
        }
        Err(SegCacheError::Exists) => {
            increment_counter!(&Stat::CasExists);
            if reply {
                write_buffer.extend_from_slice(b"EXISTS\r\n");
            }
        }
        Err(_) => {
            increment_counter!(&Stat::CasEx);
            if reply {
                write_buffer.extend_from_slice(b"NOT_STORED\r\n");
            }
        }
    }
}

#[derive(PartialEq, Eq, Debug)]
pub struct AddRequest {
    data: BytesMut,
    key_index: (usize, usize),
    expiry: u32,
    flags: u32,
    noreply: bool,
    value_index: (usize, usize),
}

impl AddRequest {
    pub fn key(&self) -> &[u8] {
        let data: &[u8] = self.data.borrow();
        &data[self.key_index.0..self.key_index.1]
    }

    pub fn value(&self) -> &[u8] {
        let data: &[u8] = self.data.borrow();
        &data[self.value_index.0..self.value_index.1]
    }

    pub fn flags(&self) -> u32 {
        self.flags
    }

    pub fn noreply(&self) -> bool {
        self.noreply
    }

    pub fn expiry(&self) -> u32 {
        self.expiry
    }
}

pub fn process_add(
    config: &Arc<Config>,
    request: AddRequest,
    write_buffer: &mut BytesMut,
    data: &mut SegCache,
) {
    increment_counter!(&Stat::Add);
    let reply = !request.noreply();
    // convert the expiry to a delta TTL
    let ttl: u32 = match config.time().time_type() {
        TimeType::Unix => {
            let epoch = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs() as u32;
            request.expiry().wrapping_sub(epoch)
        }
        TimeType::Delta => request.expiry(),
        TimeType::Memcache => {
            if request.expiry() == 0 {
                0
            } else if request.expiry() < 60 * 60 * 24 * 30 {
                request.expiry()
            } else {
                let epoch = SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as u32;
                request.expiry().wrapping_sub(epoch)
            }
        }
    };
    #[allow(clippy::collapsible_if)]
    if data.get_no_freq_incr(request.key()).is_none()
        && data
            .insert(
                request.key(),
                request.value(),
                Some(&request.flags().to_be_bytes()),
                CoarseDuration::from_secs(ttl),
            )
            .is_ok()
    {
        increment_counter!(&Stat::AddStored);
        if reply {
            write_buffer.extend_from_slice(b"STORED\r\n");
        }
    } else {
        increment_counter!(&Stat::AddNotstored);
        if reply {
            write_buffer.extend_from_slice(b"NOT_STORED\r\n");
        }
    }
}

#[derive(PartialEq, Eq, Debug)]
pub struct ReplaceRequest {
    data: BytesMut,
    key_index: (usize, usize),
    expiry: u32,
    flags: u32,
    noreply: bool,
    value_index: (usize, usize),
}

impl ReplaceRequest {
    pub fn key(&self) -> &[u8] {
        let data: &[u8] = self.data.borrow();
        &data[self.key_index.0..self.key_index.1]
    }

    pub fn value(&self) -> &[u8] {
        let data: &[u8] = self.data.borrow();
        &data[self.value_index.0..self.value_index.1]
    }

    pub fn flags(&self) -> u32 {
        self.flags
    }

    pub fn noreply(&self) -> bool {
        self.noreply
    }

    pub fn expiry(&self) -> u32 {
        self.expiry
    }
}

pub fn process_replace(
    config: &Arc<Config>,
    request: ReplaceRequest,
    write_buffer: &mut BytesMut,
    data: &mut SegCache,
) {
    increment_counter!(&Stat::Replace);
    let reply = !request.noreply();
    // convert the expiry to a delta TTL
    let ttl: u32 = match config.time().time_type() {
        TimeType::Unix => {
            let epoch = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs() as u32;
            request.expiry().wrapping_sub(epoch)
        }
        TimeType::Delta => request.expiry(),
        TimeType::Memcache => {
            if request.expiry() == 0 {
                0
            } else if request.expiry() < 60 * 60 * 24 * 30 {
                request.expiry()
            } else {
                let epoch = SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as u32;
                request.expiry().wrapping_sub(epoch)
            }
        }
    };
    #[allow(clippy::collapsible_if)]
    if data.get_no_freq_incr(request.key()).is_some()
        && data
            .insert(
                request.key(),
                request.value(),
                Some(&request.flags().to_be_bytes()),
                CoarseDuration::from_secs(ttl),
            )
            .is_ok()
    {
        increment_counter!(&Stat::ReplaceStored);
        if reply {
            write_buffer.extend_from_slice(b"STORED\r\n");
        }
    } else {
        increment_counter!(&Stat::ReplaceNotstored);
        if reply {
            write_buffer.extend_from_slice(b"NOT_STORED\r\n");
        }
    }
}

// TODO(bmartin): consider splitting the parsing functions up, cognitive
// complexity is a little high for this function. Currently this way to re-use
// the window iterators and avoid re-parsing the buffer once into command
// specific parsing.
pub fn parse(buffer: &mut BytesMut) -> Result<Request, ParseError> {
    // no-copy borrow as a slice
    let buf: &[u8] = (*buffer).borrow();

    // check if we got a CRLF
    let mut double_byte_windows = buf.windows(CRLF_LEN);
    if let Some(command_end) = double_byte_windows.position(|w| w == CRLF) {
        // single-byte windowing to find spaces
        let mut single_byte_windows = buf.windows(1);
        if let Some(command_verb_end) = single_byte_windows.position(|w| w == b" ") {
            let verb = match &buf[0..command_verb_end] {
                b"get" => Verb::Get,
                b"gets" => Verb::Gets,
                b"set" => Verb::Set,
                b"cas" => Verb::Cas,
                b"add" => Verb::Add,
                b"replace" => Verb::Replace,
                b"delete" => Verb::Delete,
                _ => {
                    return Err(ParseError::UnknownCommand);
                }
            };
            match verb {
                Verb::Get | Verb::Gets => {
                    let mut previous = command_verb_end + 1;
                    let mut keys = Vec::new();

                    // command may have multiple keys, we need to loop until we hit
                    // a CRLF
                    loop {
                        if let Some(key_end) = single_byte_windows.position(|w| w == b" ") {
                            if key_end < command_end {
                                keys.push((previous, key_end));
                                previous = key_end + 1;
                            } else {
                                keys.push((previous, command_end));
                                break;
                            }
                        } else {
                            keys.push((previous, command_end));
                            break;
                        }
                    }

                    let data = buffer.split_to(command_end + CRLF_LEN);
                    if verb == Verb::Get {
                        Ok(Request::Get(GetRequest {
                            data,
                            key_indices: keys,
                        }))
                    } else {
                        Ok(Request::Gets(GetsRequest {
                            data,
                            key_indices: keys,
                        }))
                    }
                }
                Verb::Cas => {
                    let key_end = single_byte_windows
                        .position(|w| w == b" ")
                        .ok_or(ParseError::Incomplete)?
                        + command_verb_end
                        + 1;

                    let flags_end = single_byte_windows
                        .position(|w| w == b" ")
                        .ok_or(ParseError::Incomplete)?
                        + key_end
                        + 1;
                    let flags_str = std::str::from_utf8(&buf[(key_end + 1)..flags_end])
                        .map_err(|_| ParseError::Invalid)?;
                    let flags = flags_str.parse().map_err(|_| ParseError::Invalid)?;

                    let expiry_end = single_byte_windows
                        .position(|w| w == b" ")
                        .ok_or(ParseError::Incomplete)?
                        + flags_end
                        + 1;
                    let expiry_str = std::str::from_utf8(&buf[(flags_end + 1)..expiry_end])
                        .map_err(|_| ParseError::Invalid)?;
                    let expiry = expiry_str.parse().map_err(|_| ParseError::Invalid)?;

                    let bytes_end = single_byte_windows
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
                    let next_space = single_byte_windows
                        .position(|w| w == b" ")
                        .map(|v| v + expiry_end + 1);
                    let first_crlf = double_byte_windows
                        .position(|w| w == CRLF)
                        .ok_or(ParseError::Incomplete)?;

                    let cas_end = if let Some(next_space) = next_space {
                        // if we have both, bytes_end is before the earlier of the two
                        if next_space < first_crlf {
                            // validate that noreply isn't malformed
                            if &buf[(next_space + 1)..(first_crlf)] == b"noreply" {
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

                    if let Ok(Ok(cas)) = std::str::from_utf8(&buf[(bytes_end + 1)..cas_end])
                        .map(|v| v.parse::<u64>())
                    {
                        let data_end = first_crlf + CRLF_LEN + bytes + CRLF_LEN;
                        if buf.len() >= data_end {
                            let data = buffer.split_to(data_end);
                            Ok(Request::Cas(CasRequest {
                                data,
                                key_index: ((command_verb_end + 1), key_end),
                                flags,
                                expiry,
                                noreply,
                                cas,
                                value_index: (
                                    (first_crlf + CRLF_LEN),
                                    (first_crlf + CRLF_LEN + bytes),
                                ),
                            }))
                        } else {
                            Err(ParseError::Incomplete)
                        }
                    } else {
                        Err(ParseError::Invalid)
                    }
                }
                Verb::Set | Verb::Add | Verb::Replace => {
                    let key_end = single_byte_windows
                        .position(|w| w == b" ")
                        .ok_or(ParseError::Incomplete)?
                        + command_verb_end
                        + 1;

                    let flags_end = single_byte_windows
                        .position(|w| w == b" ")
                        .ok_or(ParseError::Incomplete)?
                        + key_end
                        + 1;
                    let flags_str = std::str::from_utf8(&buf[(key_end + 1)..flags_end])
                        .map_err(|_| ParseError::Invalid)?;
                    let flags = flags_str.parse().map_err(|_| ParseError::Invalid)?;

                    let expiry_end = single_byte_windows
                        .position(|w| w == b" ")
                        .ok_or(ParseError::Incomplete)?
                        + flags_end
                        + 1;
                    let expiry_str = std::str::from_utf8(&buf[(flags_end + 1)..expiry_end])
                        .map_err(|_| ParseError::Invalid)?;
                    let expiry = expiry_str.parse().map_err(|_| ParseError::Invalid)?;

                    // now it gets tricky, we either have "[bytes] noreply\r\n" or "[bytes]\r\n"
                    let mut double_byte_windows = buf.windows(CRLF_LEN);
                    let mut noreply = false;

                    // get the position of the next space and first CRLF
                    let next_space = single_byte_windows
                        .position(|w| w == b" ")
                        .map(|v| v + expiry_end + 1);
                    let first_crlf = double_byte_windows
                        .position(|w| w == CRLF)
                        .ok_or(ParseError::Incomplete)?;

                    let bytes_end = if let Some(next_space) = next_space {
                        // if we have both, bytes_end is before the earlier of the two
                        if next_space < first_crlf {
                            // validate that noreply isn't malformed
                            if &buf[(next_space + 1)..(first_crlf)] == b"noreply" {
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

                    if let Ok(Ok(bytes)) = std::str::from_utf8(&buf[(expiry_end + 1)..bytes_end])
                        .map(|v| v.parse::<usize>())
                    {
                        let data_end = first_crlf + CRLF_LEN + bytes + CRLF_LEN;
                        if buf.len() >= data_end {
                            let data = buffer.split_to(data_end);

                            Ok(match verb {
                                Verb::Set => Request::Set(SetRequest {
                                    data,
                                    key_index: ((command_verb_end + 1), key_end),
                                    flags,
                                    expiry,
                                    noreply,
                                    value_index: (
                                        (first_crlf + CRLF_LEN),
                                        (first_crlf + CRLF_LEN + bytes),
                                    ),
                                }),
                                Verb::Add => Request::Add(AddRequest {
                                    data,
                                    key_index: ((command_verb_end + 1), key_end),
                                    flags,
                                    expiry,
                                    noreply,
                                    value_index: (
                                        (first_crlf + CRLF_LEN),
                                        (first_crlf + CRLF_LEN + bytes),
                                    ),
                                }),
                                Verb::Replace => Request::Replace(ReplaceRequest {
                                    data,
                                    key_index: ((command_verb_end + 1), key_end),
                                    flags,
                                    expiry,
                                    noreply,
                                    value_index: (
                                        (first_crlf + CRLF_LEN),
                                        (first_crlf + CRLF_LEN + bytes),
                                    ),
                                }),
                                _ => {
                                    // we already matched on the verb before parsing to restrict cases handled
                                    // anything not covered in this match is unreachable
                                    unreachable!()
                                }
                            })
                        } else {
                            Err(ParseError::Incomplete)
                        }
                    } else {
                        Err(ParseError::Invalid)
                    }
                }
                Verb::Delete => {
                    let mut noreply = false;
                    let mut double_byte_windows = buf.windows(CRLF_LEN);
                    // get the position of the next space and first CRLF
                    let next_space = single_byte_windows
                        .position(|w| w == b" ")
                        .map(|v| v + command_verb_end + 1);
                    let first_crlf = double_byte_windows
                        .position(|w| w == CRLF)
                        .ok_or(ParseError::Incomplete)?;

                    let key_end = if let Some(next_space) = next_space {
                        // if we have both, bytes_end is before the earlier of the two
                        if next_space < first_crlf {
                            // validate that noreply isn't malformed
                            if &buf[(next_space + 1)..(first_crlf)] == b"noreply" {
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

                    let command_end = if noreply {
                        key_end + 9
                    } else {
                        key_end + CRLF_LEN
                    };

                    let data = buffer.split_to(command_end);

                    Ok(Request::Delete(DeleteRequest {
                        data,
                        key_index: ((command_verb_end + 1), key_end),
                        noreply,
                    }))
                }
            }
        } else {
            Err(ParseError::UnknownCommand)
        }
    } else {
        Err(ParseError::Incomplete)
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
            assert_eq!(parse(&mut buffer), Err(ParseError::Incomplete));
        }
    }

    #[test]
    fn parse_get() {
        for key in keys() {
            let mut buffer = BytesMut::new();
            buffer.extend_from_slice(b"get ");
            buffer.extend_from_slice(key);
            buffer.extend_from_slice(b"\r\n");
            let parsed = parse(&mut buffer);
            assert!(parsed.is_ok());
            if let Ok(Request::Get(get_request)) = parsed {
                assert_eq!(get_request.keys(), vec![key]);
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
            let parsed = parse(&mut buffer);
            assert!(parsed.is_ok());
            if let Ok(Request::Gets(gets_request)) = parsed {
                assert_eq!(gets_request.keys(), vec![key]);
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
                    let parsed = parse(&mut buffer);
                    assert!(parsed.is_ok());
                    if let Ok(Request::Set(set_request)) = parsed {
                        assert_eq!(set_request.key(), key);
                        assert_eq!(set_request.value(), value);
                        assert_eq!(set_request.flags(), flag);
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
        assert!(parse(&mut buffer).is_err());
    }

    #[test]
    // interior newlines and odd spacing for add request
    fn crash_1b() {
        let mut buffer = BytesMut::new();
        buffer.extend_from_slice(b"add 1\r\n0\r\n 0 0   1\r\n0");
        assert!(parse(&mut buffer).is_err());
    }

    #[test]
    // interior newlines and odd spacing for replace request
    fn crash_1c() {
        let mut buffer = BytesMut::new();
        buffer.extend_from_slice(b"replace 1\r\n0\r\n 0 0   1\r\n0");
        assert!(parse(&mut buffer).is_err());
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
        assert!(parse(&mut buffer).is_err());
    }

    // TODO(bmartin): add test for add / replace / delete
}
