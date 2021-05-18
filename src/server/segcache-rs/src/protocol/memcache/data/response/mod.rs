// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::buffer::Buffer;
use crate::protocol::memcache::data::MemcacheItem;
use crate::protocol::Compose;
use crate::protocol::CRLF;

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

impl Compose for MemcacheResponse {
    fn compose(self, buffer: &mut Buffer) {
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
                    buffer.extend(CRLF.as_bytes());
                    buffer.extend(&*item.value);
                    buffer.extend(CRLF.as_bytes());
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
