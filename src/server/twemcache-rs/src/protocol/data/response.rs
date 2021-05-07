// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Responses which are sent back to a client

use bytes::BytesMut;
use segcache::Item;

use super::*;

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
            Self::Deleted => buffer.extend(b"DELETED\r\n"),
            Self::End => buffer.extend(b"END\r\n"),
            Self::Exists => buffer.extend(b"EXISTS\r\n"),
            Self::Item { item, cas } => {
                buffer.extend(b"VALUE ");
                buffer.extend(item.key());
                let f = item.optional().unwrap();
                let flags: u32 = u32::from_be_bytes([f[0], f[1], f[2], f[3]]);
                if cas {
                    buffer.extend(
                        &format!(" {} {} {}", flags, item.value().len(), item.cas()).into_bytes(),
                    );
                } else {
                    buffer.extend(&format!(" {} {}", flags, item.value().len()).into_bytes());
                }
                buffer.extend(CRLF);
                buffer.extend(item.value());
                buffer.extend(CRLF);
            }
            Self::NotFound => buffer.extend(b"NOT_FOUND\r\n"),
            Self::NotStored => buffer.extend(b"NOT_STORED\r\n"),
            Self::Stored => buffer.extend(b"STORED\r\n"),
        }
    }
}
