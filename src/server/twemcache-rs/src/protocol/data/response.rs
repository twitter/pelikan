// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Responses which are sent back to a client

use bytes::BytesMut;

use super::*;

pub enum MemcacheResponse<'a> {
    Deleted,
    End,
    Exists,
    Item {
        key: &'a [u8],
        value: &'a [u8],
        flags: u32,
        cas: Option<u32>,
    },
    NotFound,
    Stored,
    NotStored,
}

impl<'a> MemcacheResponse<'a> {
    pub fn serialize(self, buffer: &mut BytesMut) {
        match self {
            Self::Deleted => buffer.extend(b"DELETED\r\n"),
            Self::End => buffer.extend(b"END\r\n"),
            Self::Exists => buffer.extend(b"EXISTS\r\n"),
            Self::Item {
                key,
                value,
                flags,
                cas,
            } => {
                buffer.extend(b"VALUE ");
                buffer.extend(key);
                if let Some(cas) = cas {
                    buffer.extend(&format!(" {} {} {}", flags, value.len(), cas).into_bytes());
                } else {
                    buffer.extend(&format!(" {} {}", flags, value.len()).into_bytes());
                }
                buffer.extend(CRLF);
                buffer.extend(value);
                buffer.extend(CRLF);
            }
            Self::NotFound => buffer.extend(b"NOT_FOUND\r\n"),
            Self::NotStored => buffer.extend(b"NOT_STORED\r\n"),
            Self::Stored => buffer.extend(b"STORED\r\n"),
        }
    }
}
