// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

#![no_main]
use libfuzzer_sys::fuzz_target;

use protocol::memcache::MemcacheRequest;
use protocol::Parse;

pub const SPACE: u8 = 32;

pub const MAX_KEY_LEN: usize = 250;

fuzz_target!(|data: &[u8]| {
    if let Ok(request) = MemcacheRequest::parse(data) {
        match request.into_inner() {
            MemcacheRequest::Get { keys } | MemcacheRequest::Gets { keys } => {
                for key in keys.iter() {
                    validate_key(key);
                }
            }
            MemcacheRequest::Set { entry, .. }
            | MemcacheRequest::Add { entry, .. }
            | MemcacheRequest::Replace { entry, .. } => {
                validate_key(entry.key());
            }
            MemcacheRequest::Cas { entry, .. } => {
                validate_key(entry.key());
            }
            MemcacheRequest::Delete { key, .. } => {
                validate_key(&key);
            }
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
