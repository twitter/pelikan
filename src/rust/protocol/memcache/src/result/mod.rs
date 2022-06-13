// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::*;
use protocol_common::ExecutionResult;
use session::Session;

pub struct MemcacheExecutionResult<Request, Response> {
    pub(crate) request: Request,
    pub(crate) response: Response,
}

impl MemcacheExecutionResult<Request, Response> {
    pub fn new(request: Request, response: Response) -> Self {
        Self { request, response }
    }
}

impl ExecutionResult<Request, Response> for MemcacheExecutionResult<Request, Response> {
    fn request(&self) -> &Request {
        &self.request
    }

    fn response(&self) -> &Response {
        &self.response
    }
}

impl Compose for MemcacheExecutionResult<Request, Response> {
    fn compose(&self, dst: &mut Session) {
        match self.request {
            Request::Get(ref req) => match self.response {
                Response::Values(ref res) => {
                    let total_keys = req.keys.len();
                    let hit_keys = res.values.len();
                    let miss_keys = total_keys - hit_keys;
                    GET_KEY_HIT.add(hit_keys as _);
                    GET_KEY_MISS.add(miss_keys as _);
                }
                _ => return Error {}.compose(dst),
            },
            Request::Gets(ref req) => match self.response {
                Response::Values(ref res) => {
                    let total_keys = req.keys.len();
                    let hit_keys = res.values.len();
                    let miss_keys = total_keys - hit_keys;
                    GETS_KEY_HIT.add(hit_keys as _);
                    GETS_KEY_MISS.add(miss_keys as _);
                }
                _ => return Error {}.compose(dst),
            },
            Request::Set(_) => match self.response {
                Response::Stored(_) => {
                    SET_STORED.increment();
                }
                Response::NotStored(_) => {
                    SET_NOT_STORED.increment();
                }
                _ => return Error {}.compose(dst),
            },
            Request::Add(_) => match self.response {
                Response::Stored(_) => {
                    ADD_STORED.increment();
                }
                Response::NotStored(_) => {
                    ADD_NOT_STORED.increment();
                }
                _ => return Error {}.compose(dst),
            },
            Request::Replace(_) => match self.response {
                Response::Stored(_) => {
                    REPLACE_STORED.increment();
                }
                Response::NotStored(_) => {
                    REPLACE_NOT_STORED.increment();
                }
                _ => return Error {}.compose(dst),
            },
            Request::Cas(_) => match self.response {
                Response::Stored(_) => {
                    CAS_STORED.increment();
                }
                Response::Exists(_) => {
                    CAS_EXISTS.increment();
                }
                Response::NotFound(_) => {
                    CAS_NOT_FOUND.increment();
                }
                _ => return Error {}.compose(dst),
            },
            Request::Append(_) => match self.response {
                Response::Stored(_) => {
                    APPEND_STORED.increment();
                }
                Response::NotStored(_) => {
                    APPEND_NOT_STORED.increment();
                }
                _ => return Error {}.compose(dst),
            },
            Request::Prepend(_) => match self.response {
                Response::Stored(_) => {
                    PREPEND_STORED.increment();
                }
                Response::NotStored(_) => {
                    PREPEND_NOT_STORED.increment();
                }
                _ => return Error {}.compose(dst),
            },
            Request::Incr(_) => match self.response {
                Response::Numeric(_) => {}
                Response::NotFound(_) => {
                    INCR_NOT_FOUND.increment();
                }
                _ => return Error {}.compose(dst),
            },
            Request::Decr(_) => match self.response {
                Response::Numeric(_) => {}
                Response::NotFound(_) => {
                    DECR_NOT_FOUND.increment();
                }
                _ => return Error {}.compose(dst),
            },
            Request::Delete(_) => match self.response {
                Response::Deleted(_) => {
                    DELETE_DELETED.increment();
                }
                Response::NotFound(_) => {
                    DELETE_NOT_FOUND.increment();
                }
                _ => return Error {}.compose(dst),
            },
            Request::FlushAll(_) => {}
            Request::Quit(_) => {}
        }
        self.response.compose(dst)
    }
}
