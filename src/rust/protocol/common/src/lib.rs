// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! A collection of protocol implementations which implement a set of common
//! traits so that the a server implementation can easily switch between
//! protocol implementations.

use session::Session;

pub const CRLF: &str = "\r\n";

pub trait Compose {
    fn compose(&self, dst: &mut Session);

    /// Indicates that the connection should be closed.
    /// Override this function as appropriate for the
    /// protocol.
    fn should_hangup(&self) -> bool {
        false
    }
}

pub trait Execute<Request, Response> {
    fn execute(&mut self, request: Request) -> ExecutionResult<Request, Response>;
}

pub struct ExecutionResult<Request, Response> {
    request: Request,
    response: Response,
}

impl<Request, Response> ExecutionResult<Request, Response> {
    pub fn new(request: Request, response: Response) -> Self {
        Self { request, response }
    }

    pub fn request(&self) -> &Request {
        &self.request
    }

    pub fn response(&self) -> &Response {
        &self.response
    }
}

#[derive(Debug, PartialEq)]
pub enum ParseError {
    Invalid,
    Incomplete,
    Unknown,
}

#[derive(Debug, PartialEq)]
pub struct ParseOk<T> {
    message: T,
    consumed: usize,
}

impl<T> ParseOk<T> {
    pub fn new(message: T, consumed: usize) -> Self {
        Self { message, consumed }
    }

    pub fn into_inner(self) -> T {
        self.message
    }

    pub fn consumed(&self) -> usize {
        self.consumed
    }
}

pub trait Parse<T> {
    fn parse(&self, buffer: &[u8]) -> Result<ParseOk<T>, ParseError>;
}
