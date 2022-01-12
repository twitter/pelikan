// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! A fuzz target which makes sure that the `Admin` protocol implementation will
//! handle arbitrary data without panicking.

#![no_main]
use libfuzzer_sys::fuzz_target;

use protocol::admin::AdminRequestParser;
use protocol::Parse;

const PARSER: AdminRequestParser = AdminRequestParser {};

fuzz_target!(|data: &[u8]| {
    let _ = PARSER.parse(data);
});
