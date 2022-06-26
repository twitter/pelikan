// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! A fuzz target which makes sure that the `RESP` protocol implementation
//! will handle arbitrary data without panicking or violating protocol specific
//! invariants.

#![no_main]
use libfuzzer_sys::fuzz_target;

use protocol_resp::*;
use protocol_common::Parse;

const MAX_KEY_LEN: usize = 128;
const MAX_VALUE_SIZE: usize = 4*4096;

fuzz_target!(|data: &[u8]| {
    let parser = RequestParser::new()
        .max_value_size(MAX_VALUE_SIZE)
        .max_key_len(MAX_KEY_LEN);

    if let Ok(request) = parser.parse(data) {
        match request.into_inner() {
            Request::Get(get) => {
                validate_key(get.key())
            }
            Request::Set(set) => {
                validate_key(set.key());
                validate_value(set.value());
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
}

fn validate_value(value: &[u8]) {
    if value.len() > MAX_VALUE_SIZE {
        panic!("key is too long");
    }
}
