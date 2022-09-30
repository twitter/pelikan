// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use protocol_common::Execute;
use protocol_http::{
    request::{ParseData, Request, RequestData},
    Headers, Response, Storage,
};

use crate::Bloom;

impl Execute<ParseData, Response> for Bloom {
    fn execute(&mut self, data: &ParseData) -> Response {
        let request = match &data.0 {
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
            Response::builder(200).body(b"")
        } else {
            Response::builder(404).body(b"")
        }
    }

    fn put(&mut self, key: &[u8], _value: &[u8], _headers: &Headers) -> Response {
        let exists = self.data.contains(key);
        self.data.insert(key);

        if exists {
            Response::builder(200).body(b"")
        } else {
            Response::builder(201).body(b"")
        }
    }

    fn delete(&mut self, _key: &[u8], _headers: &Headers) -> Response {
        let mut builder = Response::builder(405);
        builder.header("Content-Type", b"text/plain");
        builder.body(b"DELETE method not supported")
    }
}
