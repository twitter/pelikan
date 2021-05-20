// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::memcache::MemcacheEntry;
use crate::Compose;
use crate::CRLF;
use std::io::Write;

pub enum MemcacheResponse {
    Deleted,
    Exists,
    Values { entries: Box<[MemcacheEntry]>, cas: bool },
    NotFound,
    NotStored,
    Stored,
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
            Self::Exists => {
                dst.write_all(b"EXISTS\r\n");
            }
            Self::Values { entries, cas } => {
                if entries.len() == 0 {
                    dst.write_all(b"END\r\n");
                }
                for entry in entries.iter() {
                    dst.write_all(b"VALUE ");
                    dst.write_all(&*entry.key);
                    if cas {
                        dst.write_all(
                            &format!(" {} {} {}", entry.flags, entry.value.len(), entry.cas.unwrap_or(0)).into_bytes(),
                        );
                    } else {
                        dst.write_all(
                            &format!(" {} {}", entry.flags, entry.value.len()).into_bytes(),
                        );
                    }
                    dst.write_all(CRLF.as_bytes());
                    dst.write_all(&*entry.value);
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
        }
    }
}
