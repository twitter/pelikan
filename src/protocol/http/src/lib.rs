// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//!

#[macro_use]
extern crate thiserror;

mod error;
pub mod request;
pub mod response;
mod util;

pub use crate::error::Error;
pub use crate::request::Headers;
pub use crate::request::{Request, RequestData, RequestParser, ParseData};
pub use crate::response::Response;

pub type Result<T> = std::result::Result<T, Error>;
pub type ParseResult = Result<Request>;

pub trait Storage {
    fn get(&mut self, key: &[u8], headers: &Headers) -> Response;
    fn put(&mut self, key: &[u8], value: &[u8], headers: &Headers) -> Response;
    fn delete(&mut self, key: &[u8], headers: &Headers) -> Response;
}
