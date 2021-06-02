// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::ping::PingRequest;
use crate::*;

#[test]
fn ping() {
    assert!(PingRequest::parse(b"ping\r\n").is_ok());
    assert!(PingRequest::parse(b"PING\r\n").is_ok());
}

#[test]
fn incomplete() {
    if let Err(e) = PingRequest::parse(b"ping") {
        if e != ParseError::Incomplete {
            panic!("invalid parse result");
        }
    } else {
        panic!("invalid parse result");
    }
}

#[test]
fn trailing_whitespace() {
    assert!(PingRequest::parse(b"ping \r\n").is_ok())
}

#[test]
fn unknown() {
    for request in &["unknown\r\n"] {
        if let Err(e) = PingRequest::parse(request.as_bytes()) {
            if e != ParseError::UnknownCommand {
                panic!("invalid parse result");
            }
        } else {
            panic!("invalid parse result");
        }
    }
}

#[test]
fn pipelined() {
    assert!(PingRequest::parse(b"ping\r\nping\r\n").is_ok());
}
