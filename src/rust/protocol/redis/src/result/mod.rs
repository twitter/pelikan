// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::*;
use protocol_common::ExecutionResult;
use session::Session;

pub struct RedisExecutionResult<Request, Response> {
    pub(crate) request: Request,
    pub(crate) response: Response,
}

impl RedisExecutionResult<Request, Response> {
    pub fn new(request: Request, response: Response) -> Self {
        Self { request, response }
    }
}

impl ExecutionResult<Request, Response> for RedisExecutionResult<Request, Response> {
    fn request(&self) -> &Request {
        &self.request
    }

    fn response(&self) -> &Response {
        &self.response
    }
}

impl Compose for RedisExecutionResult<Request, Response> {
    fn compose(&self, dst: &mut Session) {
        match self.request {
            Request::Get(_) => match self.response {
                Response::BulkString(ref res) => {
                    if res.inner.is_some() {
                        GET_KEY_HIT.increment();
                    } else {
                        GET_KEY_MISS.increment();
                    }
                }
                _ => return Error::from("Internal error").compose(dst),
            },
            Request::Set(_) => match self.response {
                Response::SimpleString(_) => {
                    SET_STORED.increment();
                }
                Response::Error(_) => {
                    SET_NOT_STORED.increment();
                }
                _ => return Error::from("Internal error").compose(dst),
            },
        }
        self.response.compose(dst)
    }
}
