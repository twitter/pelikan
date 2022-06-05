// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! A fuzz target which makes sure that the `Memcache` protocol implementation
//! will handle arbitrary data without panicking or violating protocol specific
//! invariants.

#![no_main]
use libfuzzer_sys::fuzz_target;

use protocol_memcache::*;
use protocol_common::Parse;

const MAX_KEY_LEN: usize = 250;
const MAX_BATCH_SIZE: usize = 1024;
const MAX_VALUE_SIZE: usize = 512*1024*1024;

fuzz_target!(|data: &[u8]| {
    let parser = RequestParser::new();

    if let Ok(request) = parser.parse(data) {
        match request.into_inner() {
            Request::Get(get) => {
                if get.keys().is_empty() {
                    panic!("no keys");
                }
                if get.keys().len() > MAX_BATCH_SIZE {
                    panic!("batch size exceeds max");
                }
                for key in get.keys().iter() {
                    validate_key(key);
                }
            }
            Request::Gets(gets) => {
                if gets.keys().is_empty() {
                    panic!("no keys");
                }
                if gets.keys().len() > MAX_BATCH_SIZE {
                    panic!("batch size exceeds max");
                }
                for key in gets.keys().iter() {
                    validate_key(key);
                }
            }
            Request::Set(set) => {
                validate_key(set.key());
                validate_value(set.value());
            }
            Request::Add(add) => {
                validate_key(add.key());
                validate_value(add.value());
            }
            Request::Replace(replace) => {
                validate_key(replace.key());
                validate_value(replace.value());
            }
            Request::Append(append) => {
                validate_key(append.key());
                validate_value(append.value());
            }
            Request::Prepend(prepend) => {
                validate_key(prepend.key());
                validate_value(prepend.value());
            }
            Request::Cas(cas) => {
                validate_key(cas.key());
                validate_value(cas.value());
            }
            Request::Delete(delete) => {
                validate_key(delete.key());
            }
            Request::Incr(incr) => {
                validate_key(incr.key());
            }
            Request::Decr(decr) => {
                validate_key(decr.key());
            }
            Request::FlushAll(_) => {}
            Request::Quit(_) => {}
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

fn validate_value(value: &[u8]) {
    if value.len() > MAX_VALUE_SIZE {
        panic!("key is too long");
    }
}
