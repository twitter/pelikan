// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use assert_matches::assert_matches;
use protocol_http::{Error as ParseError, Request, RequestData, RequestParser};

fn parse_to_end(data: &[u8]) -> protocol_http::Result<Request> {
    let mut buffer = data;

    let parser = RequestParser::new();
    let request = parser.do_parse(&mut buffer)?;

    assert_eq!(buffer.len(), 0);
    assert!(
        buffer.as_ptr() == data.as_ptr().wrapping_add(data.len()),
        "buffer did not point to end of data slice"
    );

    Ok(request)
}

#[test]
fn parse_get() {
    let data: &[u8] = b"\
        GET /test HTTP/1.1\r\n\
        Server: no\r\n\
        \r\n\
    ";

    let request = parse_to_end(data).expect("failed to parse request");
    let key = assert_matches!(request.data(), RequestData::Get(key) => key);

    assert_eq!(key, b"/test");
}

#[test]
fn parse_put() {
    let data: &[u8] = b"\
        PUT /test HTTP/1.1\r\n\
        Content-Length: 10\r\n\
        Test: blah\r\n\
        \r\n\
        abcdefghij\
    ";

    let request = parse_to_end(data).expect("failed to parse request");
    let (key, value) =
        assert_matches!(request.data(), RequestData::Put(key, value) => (key, value));

    assert_eq!(key, b"/test");
    assert_eq!(value, b"abcdefghij");
}

#[test]
fn parse_delete() {
    let data: &[u8] = b"\
        DELETE /test HTTP/1.1\r\n\
        Test: blah\r\n\
        \r\n\
    ";

    let request = parse_to_end(data).expect("failed to parse request");
    let key = assert_matches!(request.data(), RequestData::Delete(key) => key);

    assert_eq!(key, b"/test");
}

#[test]
fn parse_header_case_insensitive() {
    let data: &[u8] = b"\
        GET /blah HTTP/1.1\r\n\
        Test: yes\r\n\
        \r\n\
    ";

    let request = parse_to_end(data).expect("failed to parse request");

    assert_eq!(request.header("test"), Some("yes".as_bytes()));
    assert_eq!(request.header("Test"), Some("yes".as_bytes()));
    assert_eq!(request.header("tEsT"), Some("yes".as_bytes()));
}

#[test]
fn parse_key_urlencoded() {
    let parser = RequestParser::new();
    let mut data: &[u8] = b"\
        GET /%21%40%23%24%25%5E%26%2A%28%29 HTTP/1.1\r\n\
        \r\n\
    ";

    let request = parser.do_parse(&mut data).expect("failed to parse request");
    let key = assert_matches!(request.data(), RequestData::Get(key) => key);

    assert_eq!(key, b"/!@#$%^&*()");
}

#[test]
fn parse_incomplete() {
    let data: &[u8] = b"\
        PUT /aaaaaa HTTP/1.1\r\n\
        Content-Length: 100\r\n\
        \r\n\
    ";
    let result = parse_to_end(data);

    assert_matches!(result, Err(ParseError::PartialRequest(Some(100))));
}
