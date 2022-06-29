// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! A fuzz target which makes sure that the `Admin` protocol implementation will
//! handle arbitrary data without panicking.

#![no_main]
use libfuzzer_sys::fuzz_target;

use protocol_common::Parse;
use protocol_thrift::*;

const MAX_LEN: usize = 1024;

const PARSER: MessageParser = MessageParser::new(MAX_LEN);

fuzz_target!(|data: &[u8]| {
    if let Ok(message) = PARSER.parse(data) {
        let consumed = message.consumed();
        let message = message.into_inner();

        assert!(message.len() <= MAX_LEN);
        assert_eq!(message.len(), consumed - 4);
    }
});
