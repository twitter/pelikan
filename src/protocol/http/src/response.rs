// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use arrayvec::ArrayVec;
use phf::{phf_map, Map};
use protocol_common::{BufMut, Compose};
use std::io::Write;

pub struct Response {
    builder: ResponseBuilder,
    body: Option<Vec<u8>>,
}

impl Response {
    pub fn builder(status: u16) -> ResponseBuilder {
        ResponseBuilder::new(status)
    }
}

pub struct ResponseBuilder {
    headers: Vec<u8>,
    close: bool,
}

impl ResponseBuilder {
    pub fn new(status: u16) -> Self {
        let mut data = Vec::with_capacity(2048);
        write!(
            &mut data,
            "HTTP/1.1 {} {}\r\n",
            status,
            STATUSES.get(&status).copied().unwrap_or("")
        )
        .unwrap();

        Self {
            headers: data,
            close: false,
        }
    }

    pub fn header(&mut self, key: &str, value: &[u8]) -> &mut Self {
        assert!(!self.headers.is_empty());

        self.headers.extend_from_slice(key.as_bytes());
        self.headers.extend_from_slice(b": ");
        self.headers.extend_from_slice(value);
        self.headers.extend_from_slice(b"\r\n");

        self
    }

    pub fn should_close(&mut self, close: bool) -> &mut Self {
        self.close = close;
        self
    }

    /// Build a response with no body
    pub fn empty(&mut self) -> Response {
        assert!(!self.headers.is_empty());

        Response {
            builder: self.take(),
            body: None,
        }
    }

    /// Build a response with the specified body, also appends a Content-Length
    /// header.
    pub fn body(&mut self, body: &[u8]) -> Response {
        assert!(!self.headers.is_empty());

        let body = body.to_owned();
        Response {
            builder: self.take(),
            body: Some(body),
        }
    }

    fn take(&mut self) -> Self {
        Self {
            headers: std::mem::take(&mut self.headers),
            close: self.close,
        }
    }
}

impl Compose for Response {
    fn compose(&self, dst: &mut dyn BufMut) -> usize {
        let mut dst = crate::util::CountingBuf::new(dst);

        dst.put_slice(&self.builder.headers);

        if self.builder.close {
            dst.put_slice(b"Connection: close\r\n");
        } else {
            dst.put_slice(b"Connection: keep-alive\r\n");
            dst.put_slice(b"Keep-Alive: timeout=60\r\n");
        }

        if let Some(body) = &self.body {
            let mut lenbuf = ArrayVec::<u8, 24>::new();
            write!(&mut lenbuf, "{}", body.len()).unwrap();

            dst.put_slice(b"Content-Length: ");
            dst.put_slice(&lenbuf);
            dst.put_slice(b"\r\n");
        }

        dst.put_slice(b"\r\n");

        if let Some(body) = &self.body {
            dst.put_slice(body);
        }

        dst.count()
    }

    fn should_hangup(&self) -> bool {
        self.builder.close
    }
}

const STATUSES: Map<u16, &'static str> = phf_map! {
    // Informational Responses
    100u16 => "Continue",
    101u16 => "Switching Protocols",
    102u16 => "Processing",
    103u16 => "Early Hints",

    // Successful Responses
    200u16 => "OK",
    201u16 => "Created",
    202u16 => "Accepted",
    203u16 => "Non-Authoritative Information",
    204u16 => "No Content",
    205u16 => "Reset Content",
    206u16 => "Partial Content",
    207u16 => "Multi-Status",
    208u16 => "Already Reported",
    226u16 => "IM Used",

    // Redirect Responses
    300u16 => "Multiple Choices",
    301u16 => "Moved Permanently",
    302u16 => "Found",
    303u16 => "See Other",
    304u16 => "Not Modified",
    305u16 => "Use Proxy",
    307u16 => "Temporary Redirect",
    308u16 => "Permanent Redirect",

    // Client Error Responses
    400u16 => "Bad Request",
    401u16 => "Unauthorized",
    402u16 => "Payment Required",
    403u16 => "Forbidden",
    404u16 => "Not Found",
    405u16 => "Method Not Allowed",
    406u16 => "Not Acceptable",
    407u16 => "Proxy Authentication Required",
    408u16 => "Request Timeout",
    409u16 => "Conflict",
    410u16 => "Gone",
    411u16 => "Length Required",
    412u16 => "Precondition Failed",
    413u16 => "Payload Too Large",
    414u16 => "URI Too Long",
    415u16 => "Unsupported Media Type",
    416u16 => "Range Not Satisfiable",
    417u16 => "Expectation Mailed",
    418u16 => "I'm a Teapot",
    421u16 => "Misdirected Request",
    422u16 => "Unprocessable Entity",
    423u16 => "Locked",
    424u16 => "Failed Dependency",
    425u16 => "Too Early",
    426u16 => "Upgrade Required",
    428u16 => "Precondition Required",
    429u16 => "Too Many Requests",
    431u16 => "Request Header Field Too Large",
    451u16 => "Unavailable For Legal Reasons",

    // Server Error Responses
    500u16 => "Internal Server Error",
    501u16 => "Not Implemented",
    502u16 => "Bad Gateway",
    503u16 => "Service Unavailable",
    504u16 => "Gateway Timeout",
    505u16 => "HTTP Version Not Supported",
    506u16 => "Variant Also Negotiates",
    507u16 => "Insufficient Storage",
    508u16 => "Loop Detected",
    510u16 => "Not Extended",
    511u16 => "Network Authentication Required",
};
