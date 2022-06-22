// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Implements the wire protocol for the `Ping` protocol implementation.

mod request;
mod response;

use protocol_common::Compose;
use protocol_common::ExecutionResult;
pub use request::*;
pub use response::*;
use session::Session;

#[allow(unused)]
use rustcommon_metrics::*;

counter!(PING);
counter!(PONG);

pub struct PingExecutionResult<Request, Response> {
    request: Request,
    response: Response,
}

impl PingExecutionResult<Request, Response> {
    pub fn new(request: Request, response: Response) -> Self {
        Self { request, response }
    }
}

impl ExecutionResult<Request, Response> for PingExecutionResult<Request, Response> {
    fn request(&self) -> &Request {
        &self.request
    }

    fn response(&self) -> &Response {
        &self.response
    }
}

impl Compose for PingExecutionResult<Request, Response> {
    fn compose(&self, dst: &mut Session) {
        PONG.increment();
        self.response.compose(dst)
    }
}
