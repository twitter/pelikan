// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Implements the serialization of `Memcache` protocol responses into the wire
//! representation.

use super::*;
use crate::memcache::wire::MemcacheCommand;
use crate::memcache::MemcacheEntry;
use crate::memcache::MemcacheRequest;
use crate::Compose;
use crate::CRLF;
use session::Session;
use std::borrow::Cow;
use std::fmt::Debug;
use std::io::Write;
use storage_types::Value;

pub struct MemcacheResponse {
    pub request: MemcacheRequest,
    pub result: MemcacheResult,
}

pub enum MemcacheResult {
    Deleted,
    Exists,
    Values {
        entries: Box<[MemcacheEntry]>,
        cas: bool,
    },
    NotFound,
    NotStored,
    Stored,
    Error,
    Count(u64),
}

impl Debug for MemcacheResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        let name = match self {
            Self::Deleted => "Deleted",
            Self::Exists => "Exists",
            Self::Values { .. } => "Values",
            Self::NotFound => "NotFound",
            Self::NotStored => "NotStored",
            Self::Stored => "Stored",
            Self::Error => "Error",
            Self::Count(_) => "Count",
        };
        write!(f, "MemcacheResult::{}", name)
    }
}

impl MemcacheResult {
    fn len(&self) -> usize {
        self.as_bytes().len()
    }

    fn as_bytes(&self) -> &[u8] {
        match self {
            Self::Deleted => b"DELETED\r\n",
            Self::Exists => b"EXISTS\r\n",
            Self::Values { .. } => b"VALUE ",
            Self::NotFound => b"NOT_FOUND\r\n",
            Self::NotStored => b"NOT_STORED\r\n",
            Self::Stored => b"STORED\r\n",
            Self::Error => b"ERROR\r\n",
            Self::Count(_) => b"",
        }
    }

    /// Returns the numeric code representing the result of the request. This is
    /// used in the command log and maintains backwards compatibility by
    /// mirroring the codes from:
    /// https://github.com/twitter/pelikan/blob/master/src/protocol/data/memcache/response.h
    fn code(&self) -> usize {
        match self {
            // UNKNOWN
            // OK
            // END
            // STAT
            // VALUE
            Self::Stored => 5,
            Self::Exists => 6,
            Self::Deleted => 7,
            Self::NotFound => 8,
            Self::NotStored => 9,
            // CLIENT_ERROR
            // SERVER_ERROR
            _ => usize::MAX,
        }
    }
}

// TODO(bmartin): consider a different trait bound here when reworking buffers.
// We ignore the unused result warnings here because we know we're using a
// buffer with infallible writes (growable buffer). This is *not* guaranteed by
// the current trait bound.
#[allow(unused_must_use)]
impl Compose for MemcacheResponse {
    fn compose(self, dst: &mut Session) {
        match self.request {
            MemcacheRequest::Get { .. } => {
                GET.increment();
            }
            MemcacheRequest::Gets { .. } => {
                GETS.increment();
            }
            MemcacheRequest::Set { .. } => {
                match self.result {
                    MemcacheResult::Stored => {
                        SET_STORED.increment();
                    }
                    MemcacheResult::NotStored => {
                        SET_NOT_STORED.increment();
                    }
                    _ => unreachable!(),
                }
                SET.increment();
            }
            MemcacheRequest::Add { .. } => {
                match self.result {
                    MemcacheResult::Stored => {
                        ADD_STORED.increment();
                    }
                    MemcacheResult::NotStored => {
                        ADD_NOT_STORED.increment();
                    }
                    _ => unreachable!(),
                }
                ADD.increment();
            }
            MemcacheRequest::Replace { .. } => {
                match self.result {
                    MemcacheResult::Stored => {
                        REPLACE_STORED.increment();
                    }
                    MemcacheResult::NotStored => {
                        REPLACE_NOT_STORED.increment();
                    }
                    _ => unreachable!(),
                }
                REPLACE.increment();
            }
            MemcacheRequest::Append { .. } => {
                match self.result {
                    MemcacheResult::Stored => {
                        APPEND_STORED.increment();
                    }
                    MemcacheResult::NotStored => {
                        APPEND_NOT_STORED.increment();
                    }
                    MemcacheResult::Error => {
                        APPEND_EX.increment();
                    }
                    _ => {
                        error!("didn't expect: {:?} for {:?}", self.result, self.request);
                        unreachable!()
                    }
                }
                APPEND.increment();
            }
            MemcacheRequest::Prepend { .. } => {
                match self.result {
                    MemcacheResult::Stored => {
                        PREPEND_STORED.increment();
                    }
                    MemcacheResult::NotStored => {
                        PREPEND_NOT_STORED.increment();
                    }
                    MemcacheResult::Error => {
                        PREPEND_EX.increment();
                    }
                    _ => {
                        error!("didn't expect: {:?} for {:?}", self.result, self.request);
                        unreachable!()
                    }
                }
                PREPEND.increment();
            }
            MemcacheRequest::Delete { .. } => {
                match self.result {
                    MemcacheResult::NotFound => {
                        DELETE_NOT_FOUND.increment();
                    }
                    MemcacheResult::Deleted => {
                        DELETE_DELETED.increment();
                    }
                    _ => {
                        error!("didn't expect: {:?} for {:?}", self.result, self.request);
                        unreachable!()
                    }
                }
                DELETE.increment();
            }
            MemcacheRequest::Incr { .. } => {
                match self.result {
                    MemcacheResult::NotFound => {
                        INCR_NOT_FOUND.increment();
                    }
                    MemcacheResult::Error => {
                        INCR_EX.increment();
                    }
                    MemcacheResult::Count { .. } => {}
                    _ => {
                        error!("didn't expect: {:?} for {:?}", self.result, self.request);
                        unreachable!()
                    }
                }
                INCR.increment();
            }
            MemcacheRequest::Decr { .. } => {
                match self.result {
                    MemcacheResult::NotFound => {
                        DECR_NOT_FOUND.increment();
                    }
                    MemcacheResult::Error => {
                        DECR_EX.increment();
                    }
                    MemcacheResult::Count { .. } => {}
                    _ => {
                        error!("didn't expect: {:?} for {:?}", self.result, self.request);
                        unreachable!()
                    }
                }
                DECR.increment();
            }
            MemcacheRequest::Cas { .. } => {
                match self.result {
                    MemcacheResult::Exists => {
                        CAS_EXISTS.increment();
                    }
                    MemcacheResult::NotFound => {
                        CAS_NOT_FOUND.increment();
                    }
                    MemcacheResult::NotStored => {
                        CAS_EX.increment();
                    }
                    MemcacheResult::Stored => {
                        CAS_STORED.increment();
                    }
                    _ => unreachable!(),
                }
                CAS.increment();
            }
            MemcacheRequest::FlushAll => {}
        }
        if let MemcacheResult::Values { ref entries, cas } = self.result {
            let mut hits = 0;
            let total = entries.len();

            for entry in entries.iter() {
                let response_len = if let Some(value) = entry.value() {
                    hits += 1;
                    let start_len = dst.write_pending();
                    dst.write_all(self.result.as_bytes());
                    dst.write_all(&*entry.key);

                    match value {
                        Value::Bytes(value) => {
                            if cas {
                                write!(
                                    dst,
                                    " {} {} {}",
                                    entry.flags,
                                    value.len(),
                                    entry.cas.unwrap_or(0)
                                )
                            } else {
                                write!(dst, " {} {}", entry.flags, value.len())
                            };
                            dst.write_all(CRLF.as_bytes());
                            dst.write_all(value);
                        }
                        Value::U64(value) => {
                            let value_string = format!("{}", value);
                            let value = value_string.as_bytes();
                            if cas {
                                write!(
                                    dst,
                                    " {} {} {}",
                                    entry.flags,
                                    value.len(),
                                    entry.cas.unwrap_or(0)
                                )
                            } else {
                                write!(dst, " {} {}", entry.flags, value.len())
                            };
                            dst.write_all(CRLF.as_bytes());
                            dst.write_all(value);
                        }
                    };

                    dst.write_all(CRLF.as_bytes());

                    // return the number of bytes in the reply
                    dst.write_pending() - start_len
                } else {
                    0
                };
                klog_get(&self.request.command(), entry.key(), response_len);
            }
            if self.request.command() == MemcacheCommand::Get {
                GET_KEY.add(total as _);
                GET_KEY_HIT.add(hits as _);
                GET_KEY_MISS.add((total - hits) as _);
            } else {
                GETS_KEY.add(total as _);
                GETS_KEY_HIT.add(hits as _);
                GETS_KEY_MISS.add((total - hits) as _);
            }

            dst.write_all(b"END\r\n");
        } else if let MemcacheResult::Count(c) = self.result {
            let response_len = if self.request.noreply() {
                0
            } else {
                let response = format!("{}\r\n", c);
                dst.write_all(response.as_bytes());
                response.len()
            };

            match self.request.command() {
                MemcacheCommand::Incr => klog_delta(&self, response_len),
                MemcacheCommand::Decr => klog_delta(&self, response_len),
                _ => unreachable!(),
            }
        } else {
            let response_len = if self.request.noreply() {
                0
            } else {
                dst.write_all(self.result.as_bytes());
                self.result.len()
            };

            match self.request.command() {
                MemcacheCommand::Delete => klog_delete(&self, response_len),
                MemcacheCommand::Cas => klog_cas(&self, response_len),
                _ => klog_store(&self, response_len),
            }
        }
    }
}

////////////////////////////////////////////////////////////////////////////////
// Helper Functions
////////////////////////////////////////////////////////////////////////////////

/// Transform
fn string_key(key: Result<&[u8], ()>) -> Cow<'_, str> {
    String::from_utf8_lossy(key.unwrap_or(b""))
}

/// Logs a CAS command
fn klog_cas(response: &MemcacheResponse, response_len: usize) {
    if let Some(entry) = response.request.entry() {
        klog!(
            "\"{} {} {} {} {} {}\" {} {}",
            response.request.command(),
            string_key(response.request.key()),
            entry.flags(),
            entry.ttl.map(|v| v.as_secs()).unwrap_or(0),
            entry.value().map(|v| v.len()).unwrap_or(0),
            entry.cas().unwrap_or(0) as u32,
            response.result.code(),
            response_len
        );
    } else {
        debug!("{} request missing entry", response.request.command());
    }
}

/// Logs a DELETE command
fn klog_delete(response: &MemcacheResponse, response_len: usize) {
    klog!(
        "\"{} {}\" {} {}",
        response.request.command(),
        string_key(response.request.key()),
        response.result.code(),
        response_len
    );
}

/// Logs a INCR or DECR command
fn klog_delta(response: &MemcacheResponse, response_len: usize) {
    klog!(
        "\"{} {}\" {} {}",
        response.request.command(),
        string_key(response.request.key()),
        response.result.code(),
        response_len
    );
}

/// Logs GET or GETS
fn klog_get(command: &MemcacheCommand, key: &[u8], response_len: usize) {
    if response_len == 0 {
        klog!("\"{} {}\" 0 {}", command, string_key(Ok(key)), response_len);
    } else {
        klog!("\"{} {}\" 4 {}", command, string_key(Ok(key)), response_len);
    }
}

/// Logs SET, ADD, or REPLACE
fn klog_store(response: &MemcacheResponse, response_len: usize) {
    if let Some(entry) = response.request.entry() {
        klog!(
            "\"{} {} {} {} {}\" {} {}",
            response.request.command(),
            string_key(response.request.key()),
            entry.flags(),
            entry.ttl.map(|v| v.as_secs()).unwrap_or(0),
            entry.value().map(|v| v.len()).unwrap_or(0),
            response.result.code(),
            response_len
        );
    } else {
        debug!("{} request missing entry", response.request.command());
    }
}
