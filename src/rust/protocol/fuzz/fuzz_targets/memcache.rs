// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! A fuzz target which makes sure that the `Memcache` protocol implementation
//! will handle arbitrary data without panicking or violating protocol specific
//! invariants.

#![no_main]
use libfuzzer_sys::fuzz_target;

use config::TimeType;
use protocol::memcache::{MemcacheRequest, MemcacheRequestParser};
use protocol::Parse;

const MAX_KEY_LEN: usize = 250;
const MAX_BATCH_SIZE: usize = 1024;
const MAX_VALUE_SIZE: usize = 1024*1024;

fuzz_target!(|data: &[u8]| {
    let parser = MemcacheRequestParser::new(MAX_VALUE_SIZE, TimeType::Memcache);

    if let Ok(request) = parser.parse(data) {
        match request.into_inner() {
            MemcacheRequest::Get { keys } | MemcacheRequest::Gets { keys } => {
                if keys.is_empty() {
                    panic!("no keys");
                }
                if keys.len() > MAX_BATCH_SIZE {
                    panic!("too many keys");
                }
                for key in keys.iter() {
                    validate_key(key);
                }
            }
            MemcacheRequest::Set { entry, .. }
            | MemcacheRequest::Add { entry, .. }
            | MemcacheRequest::Replace { entry, .. }
            | MemcacheRequest::Append { entry, .. }
            | MemcacheRequest::Prepend { entry, .. } => {
                validate_key(entry.key());
                if entry.value().map(|v| v.len()).unwrap_or(0) > MAX_VALUE_SIZE {
                    panic!("value too long");
                }
            }
            MemcacheRequest::Cas { entry, .. } => {
                validate_key(entry.key());
                if entry.value().map(|v| v.len()).unwrap_or(0) > MAX_VALUE_SIZE {
                    panic!("value too long");
                }
            }
            MemcacheRequest::Delete { key, .. }
            | MemcacheRequest::Incr { key, .. }
            | MemcacheRequest::Decr { key, .. } => {
                validate_key(&key);
            }
            MemcacheRequest::FlushAll => {}
        }
    }
});

fn validate_key(key: &[u8]) {
    if key.is_empty() {
        panic!("key is zero-length");
    }
    if key.len() > MAX_KEY_LEN {
        panic!("key is too long");
    }
    if key.windows(1).any(|w| w == b" ") {
        panic!("key contains SPACE: {:?}", key);
    }
    if key.windows(2).any(|w| w == b"\r\n") {
        panic!("key contains CRLF: {:?}", key);
    }
}
