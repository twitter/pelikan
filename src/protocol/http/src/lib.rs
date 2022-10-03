// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! HTTP protocol for pelikan.
//! 
//! This crate contains definitions for a basic REST protocol for interacting
//! with a cache. It supports just 3 operations:
//! - `GET` - get the value associated with the provided key, if present
//! - `PUT` - set the value associated with the provided key
//! - `DELETE` - remove a key from the cache
//! 
//! In all cases the key is passed in as the request path in the request
//! and the value is passed in as the request body. The protocol supports
//! reusing the HTTP connection for multiple requests. The only length
//! specification supported by pelikan is setting the Content-Length header.

#[macro_use]
extern crate thiserror;

mod error;
pub mod request;
pub mod response;
mod util;

pub use crate::error::Error;
pub use crate::request::Headers;
pub use crate::request::{ParseData, Request, RequestData, RequestParser};
pub use crate::response::Response;

pub type Result<T> = std::result::Result<T, Error>;
pub type ParseResult = Result<Request>;

pub trait Storage {
    fn get(&mut self, key: &[u8], headers: &Headers) -> Response;
    fn put(&mut self, key: &[u8], value: &[u8], headers: &Headers) -> Response;
    fn delete(&mut self, key: &[u8], headers: &Headers) -> Response;
}
