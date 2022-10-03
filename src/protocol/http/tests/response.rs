// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use bstr::BStr;
use protocol_common::Compose;
use protocol_http::Response;

#[test]
fn response_with_body() {
    let body = b"TEST BODY";
    let response = Response::builder(200)
        .header("Test", b"test")
        .should_close(true)
        .body(body);

    let mut data = Vec::new();
    response.compose(&mut data);

    assert_eq!(
        BStr::new(&data),
        BStr::new(
            b"\
                HTTP/1.1 200 OK\r\n\
                Test: test\r\n\
                Connection: close\r\n\
                Content-Length: 9\r\n\
                \r\n\
                TEST BODY\
            "
        )
    );
}

#[test]
fn response_empty() {
    let response = Response::builder(418)
        .header("Test", b"test")
        .should_close(false)
        .empty();

    let mut data = Vec::new();
    response.compose(&mut data);

    assert_eq!(
        BStr::new(&data),
        BStr::new(
            b"\
                HTTP/1.1 418 I'm a Teapot\r\n\
                Test: test\r\n\
                Connection: keep-alive\r\n\
                Keep-Alive: timeout=60\r\n\
                \r\n\
            "
        )
    );
}
