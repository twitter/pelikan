// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Tests for the `Ping` protocol implementation.

use crate::ping::PingRequestParser;
use crate::*;

#[test]
fn ping() {
    let parser = PingRequestParser::new();

    assert!(parser.parse(b"ping\r\n").is_ok());
    assert!(parser.parse(b"PING\r\n").is_ok());
}

#[test]
fn incomplete() {
    let parser = PingRequestParser::new();

    if let Err(e) = parser.parse(b"ping") {
        if e != ParseError::Incomplete {
            panic!("invalid parse result");
        }
    } else {
        panic!("invalid parse result");
    }
}

#[test]
fn trailing_whitespace() {
    let parser = PingRequestParser::new();

    assert!(parser.parse(b"ping \r\n").is_ok())
}

#[test]
fn unknown() {
    let parser = PingRequestParser::new();

    for request in &["unknown\r\n"] {
        if let Err(e) = parser.parse(request.as_bytes()) {
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
    let parser = PingRequestParser::new();

    assert!(parser.parse(b"ping\r\nping\r\n").is_ok());
}
