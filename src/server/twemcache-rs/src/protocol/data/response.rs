// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Responses which are sent back to a client

use bytes::BytesMut;

use super::*;

pub struct MemcacheItem {
    pub key: Box<[u8]>,
    pub value: Box<[u8]>,
    pub flags: u32,
    pub cas: Option<u32>,
}

pub enum MemcacheResponse {
    Deleted,
    End,
    Exists,
    Items(Box<[MemcacheItem]>),
    NotFound,
    Stored,
    NotStored,
    NoReply,
}

impl MemcacheResponse {
    pub fn serialize(self, buffer: &mut BytesMut) {
        match self {
            Self::Deleted => buffer.extend(b"DELETED\r\n"),
            Self::End => buffer.extend(b"END\r\n"),
            Self::Exists => buffer.extend(b"EXISTS\r\n"),
            Self::Items(items) => {
                for item in items.iter() {
                    buffer.extend(b"VALUE ");
                    buffer.extend(&*item.key);
                    if let Some(cas) = item.cas {
                        buffer.extend(
                            &format!(" {} {} {}", item.flags, item.value.len(), cas).into_bytes(),
                        );
                    } else {
                        buffer
                            .extend(&format!(" {} {}", item.flags, item.value.len()).into_bytes());
                    }
                    buffer.extend(CRLF);
                    buffer.extend(&*item.value);
                    buffer.extend(CRLF);
                }
                buffer.extend(b"END\r\n");
            }
            Self::NotFound => buffer.extend(b"NOT_FOUND\r\n"),
            Self::NotStored => buffer.extend(b"NOT_STORED\r\n"),
            Self::Stored => buffer.extend(b"STORED\r\n"),
            Self::NoReply => {}
        }
    }
}
