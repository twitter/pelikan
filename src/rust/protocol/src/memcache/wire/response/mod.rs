// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::memcache::wire::MemcacheCommand;
use crate::memcache::MemcacheEntry;
use crate::memcache::MemcacheRequest;
use crate::Compose;
use crate::CRLF;
use metrics::Stat;
use session::Session;
use std::borrow::Cow;
use std::io::Write;

pub struct MemcacheResponse {
    pub(crate) request: MemcacheRequest,
    pub(crate) result: MemcacheResult,
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
            MemcacheRequest::Get { .. } => increment_counter!(&Stat::Get),
            MemcacheRequest::Gets { .. } => increment_counter!(&Stat::Gets),
            MemcacheRequest::Set { .. } => {
                match self.result {
                    MemcacheResult::Stored => increment_counter!(&Stat::SetStored),
                    MemcacheResult::NotStored => increment_counter!(&Stat::SetNotstored),
                    _ => unreachable!(),
                }
                increment_counter!(&Stat::Set);
            }
            MemcacheRequest::Add { .. } => {
                match self.result {
                    MemcacheResult::Stored => increment_counter!(&Stat::AddStored),
                    MemcacheResult::NotStored => increment_counter!(&Stat::AddNotstored),
                    _ => unreachable!(),
                }
                increment_counter!(&Stat::Add);
            }
            MemcacheRequest::Replace { .. } => {
                match self.result {
                    MemcacheResult::Stored => increment_counter!(&Stat::ReplaceStored),
                    MemcacheResult::NotStored => increment_counter!(&Stat::ReplaceNotstored),
                    _ => unreachable!(),
                }
                increment_counter!(&Stat::Replace);
            }
            MemcacheRequest::Delete { .. } => {
                match self.result {
                    MemcacheResult::NotFound => increment_counter!(&Stat::DeleteNotfound),
                    MemcacheResult::Deleted => increment_counter!(&Stat::DeleteDeleted),
                    _ => unreachable!(),
                }
                increment_counter!(&Stat::Delete);
            }
            MemcacheRequest::Cas { .. } => {
                match self.result {
                    MemcacheResult::Exists => increment_counter!(&Stat::CasExists),
                    MemcacheResult::NotFound => increment_counter!(&Stat::CasNotfound),
                    MemcacheResult::NotStored => increment_counter!(&Stat::CasEx),
                    MemcacheResult::Stored => increment_counter!(&Stat::CasStored),
                    _ => unreachable!(),
                }
                increment_counter!(&Stat::Cas);
            }
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
                    dst.write_all(CRLF.as_bytes());

                    // return the number of bytes in the reply
                    dst.write_pending() - start_len
                } else {
                    0
                };
                klog_get(&self.request.command(), entry.key(), response_len);
            }
            if self.request.command() == MemcacheCommand::Get {
                increment_counter_by!(&Stat::GetKey, total as u64);
                increment_counter_by!(&Stat::GetKeyHit, hits as u64);
                increment_counter_by!(&Stat::GetKeyMiss, (total - hits) as u64);
            } else {
                increment_counter_by!(&Stat::GetsKey, total as u64);
                increment_counter_by!(&Stat::GetsKeyHit, hits as u64);
                increment_counter_by!(&Stat::GetsKeyMiss, (total - hits) as u64);
            }

            dst.write_all(b"END\r\n");
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
    String::from_utf8_lossy(key.unwrap_or_else(|_| b""))
}

/// Logs a CAS command
fn klog_cas(response: &MemcacheResponse, response_len: usize) {
    if let Some(entry) = response.request.entry() {
        klog!(
            "\"{} {} {} {} {} {}\" {} {}",
            response.request.command(),
            string_key(response.request.key()),
            entry.flags(),
            entry.ttl.unwrap_or(0),
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
            entry.ttl.unwrap_or(0),
            entry.value().map(|v| v.len()).unwrap_or(0),
            response.result.code(),
            response_len
        );
    } else {
        debug!("{} request missing entry", response.request.command());
    }
}
