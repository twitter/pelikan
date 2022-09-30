// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use std::borrow::Cow;

use protocol_common::Execute;
use protocol_http::{
    request::{Request, RequestData},
    Headers, ParseResult, Response, Storage,
};

use crate::Bloom;

impl Execute<ParseResult, Response> for Bloom {
    fn execute(&mut self, result: &ParseResult) -> Response {
        let request = match result {
            Ok(request) => request,
            Err(e) => return e.to_response(),
        };

        let Request { headers, data } = request;
        match data {
            RequestData::Get(key) => self.get(key, headers),
            RequestData::Put(key, value) => self.put(key, value, headers),
            RequestData::Delete(key) => self.delete(key, headers),
        }
    }
}

impl Storage for Bloom {
    fn get(&mut self, key: &[u8], _headers: &Headers) -> Response {
        if self.data.contains(key) {
            Response::builder(204).empty()
        } else {
            Response::builder(404).empty()
        }
    }

    fn put(&mut self, key: &[u8], _value: &[u8], _headers: &Headers) -> Response {
        self.data.insert(key);
        Response::builder(204).empty()
    }

    fn delete(&mut self, _key: &[u8], _headers: &Headers) -> Response {
        let mut builder = Response::builder(405);
        builder.header("Content-Type", b"text/plain");
        builder.body(Cow::Borrowed(b"DELETE method not supported"))
    }
}
