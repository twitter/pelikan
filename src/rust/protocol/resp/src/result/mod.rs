// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::*;
use protocol_common::ExecutionResult;
use session::Session;

pub struct RespExecutionResult<Request, Response> {
    pub(crate) request: Request,
    pub(crate) response: Response,
}

impl RespExecutionResult<Request, Response> {
    pub fn new(request: Request, response: Response) -> Self {
        Self { request, response }
    }
}

impl ExecutionResult<Request, Response> for RespExecutionResult<Request, Response> {
    fn request(&self) -> &Request {
        &self.request
    }

    fn response(&self) -> &Response {
        &self.response
    }
}

impl Compose for RespExecutionResult<Request, Response> {
    fn compose(&self, dst: &mut Session) {
        match self.request {
            Request::Get(_) => match self.response {
                Response::BulkString(ref res) => {
                    if res.inner.is_some() {
                        COMPOSE_GET_HIT.increment();
                    } else {
                        COMPOSE_GET_MISS.increment();
                    }
                }
                _ => return Error::from("Internal error").compose(dst),
            },
            Request::Set(_) => match self.response {
                Response::SimpleString(_) => {
                    COMPOSE_SET_STORED.increment();
                }
                Response::Error(_) => {
                    COMPOSE_SET_NOT_STORED.increment();
                }
                _ => return Error::from("Internal error").compose(dst),
            },
        }
        self.response.compose(dst)
    }
}
