// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Tests for the `Ping` protocol implementation.

use crate::*;
use std::io::ErrorKind;

#[test]
fn ping() {
    let parser = ResponseParser::new();

    assert!(parser.parse(b"pong\r\n").is_ok());
    assert!(parser.parse(b"PONG\r\n").is_ok());
}

#[test]
fn incomplete() {
    let parser = ResponseParser::new();

    if let Err(e) = parser.parse(b"pong") {
        if e.kind() != ErrorKind::WouldBlock {
            panic!("invalid parse result");
        }
    } else {
        panic!("invalid parse result");
    }
}

#[test]
fn trailing_whitespace() {
    let parser = ResponseParser::new();

    assert!(parser.parse(b"pong \r\n").is_ok())
}

#[test]
fn unknown() {
    let parser = ResponseParser::new();

    for request in &["unknown\r\n"] {
        if let Err(e) = parser.parse(request.as_bytes()) {
            if e.kind() != ErrorKind::WouldBlock {
                panic!("invalid parse result");
            }
        } else {
            panic!("invalid parse result");
        }
    }
}

#[test]
fn pipelined() {
    let parser = ResponseParser::new();

    assert!(parser.parse(b"pong\r\npong\r\n").is_ok());
}
