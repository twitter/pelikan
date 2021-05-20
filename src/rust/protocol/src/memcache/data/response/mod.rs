// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::memcache::MemcacheEntry;
// use crate::memcache::data::MemcacheItem;
use crate::Compose;
use crate::CRLF;
use std::io::Write;

/// Memcache response types
pub enum MemcacheResponse {
    Deleted,
    End,
    Exists,
    Items(Box<[MemcacheEntry]>),
    NotFound,
    Stored,
    NotStored,
    NoReply,
}

// TODO(bmartin): consider a different trait bound here when reworking buffers.
// We ignore the unused result warnings here because we know we're using a
// buffer with infallible writes (growable buffer). This is *not* guaranteed by
// the current trait bound.
#[allow(unused_must_use)]
impl Compose for MemcacheResponse {
    fn compose<Buffer: Write>(self, dst: &mut Buffer) {
        match self {
            Self::Deleted => {
                dst.write_all(b"DELETED\r\n");
            }
            Self::End => {
                dst.write_all(b"END\r\n");
            }
            Self::Exists => {
                dst.write_all(b"EXISTS\r\n");
            }
            Self::Items(items) => {
                for item in items.iter() {
                    dst.write_all(b"VALUE ");
                    dst.write_all(&*item.key);
                    if let Some(cas) = item.cas {
                        dst.write_all(
                            &format!(" {} {} {}", item.flags, item.value.len(), cas).into_bytes(),
                        );
                    } else {
                        dst.write_all(
                            &format!(" {} {}", item.flags, item.value.len()).into_bytes(),
                        );
                    }
                    dst.write_all(CRLF.as_bytes());
                    dst.write_all(&*item.value);
                    dst.write_all(CRLF.as_bytes());
                }
                dst.write_all(b"END\r\n");
            }
            Self::NotFound => {
                dst.write_all(b"NOT_FOUND\r\n");
            }
            Self::NotStored => {
                dst.write_all(b"NOT_STORED\r\n");
            }
            Self::Stored => {
                dst.write_all(b"STORED\r\n");
            }
            Self::NoReply => {}
        }
    }
}
