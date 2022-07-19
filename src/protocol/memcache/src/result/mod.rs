// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::*;
use logger::*;
use protocol_common::BufMut;
use protocol_common::ExecutionResult;
use std::borrow::Cow;
use std::ops::Deref;

// response codes for klog
const MISS: u8 = 0;
const HIT: u8 = 4;
const STORED: u8 = 5;
const EXISTS: u8 = 6;
const DELETED: u8 = 7;
const NOT_FOUND: u8 = 8;
const NOT_STORED: u8 = 9;

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
    fn compose(&self, dst: &mut dyn BufMut) {
        match self.request {
            Request::Get(ref req) => match self.response {
                Response::Values(ref res) => {
                    let total_keys = req.keys.len();
                    let hit_keys = res.values.len();
                    let miss_keys = total_keys - hit_keys;
                    GET_KEY_HIT.add(hit_keys as _);
                    GET_KEY_MISS.add(miss_keys as _);

                    let values = res.values();
                    let mut value_index = 0;

                    for key in req.keys() {
                        let key = key.deref();
                        // if we are out of values or the keys don't match, it's a miss
                        if value_index >= values.len() || values[value_index].key() != key {
                            klog!("\"get {}\" 0 0", String::from_utf8_lossy(key));
                        } else {
                            // let start = dst.len();
                            values[value_index].compose(dst);
                            // let size = dst.write_pending() - start;
                            klog!("\"get {}\" 4 {}", String::from_utf8_lossy(key), 0);
                            value_index += 1;
                        }
                    }

                    dst.put_slice(b"END\r\n");

                    return;
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

                    let values = res.values();
                    let mut value_index = 0;

                    for key in req.keys() {
                        let key = key.deref();
                        // if we are out of values or the keys don't match, it's a miss
                        if value_index >= values.len() || values[value_index].key() != key {
                            klog!("\"gets {}\" {} 0", String::from_utf8_lossy(key), MISS);
                        } else {
                            // let start = dst.write_pending();
                            values[value_index].compose(dst);
                            // let size = dst.write_pending() - start;
                            klog!("\"gets {}\" {} {}", String::from_utf8_lossy(key), HIT, 0);
                            value_index += 1;
                        }
                    }

                    dst.put_slice(b"END\r\n");

                    return;
                }
                _ => return Error {}.compose(dst),
            },
            Request::Set(ref req) => {
                let ttl: i64 = match req.ttl() {
                    None => 0,
                    Some(0) => -1,
                    Some(t) => t as _,
                };
                let (code, len) = match self.response {
                    Response::Stored(ref res) => {
                        SET_STORED.increment();
                        (STORED, res.len())
                    }
                    Response::NotStored(ref res) => {
                        SET_NOT_STORED.increment();
                        (NOT_STORED, res.len())
                    }
                    _ => return Error {}.compose(dst),
                };
                klog!(
                    "\"set {} {} {} {}\" {} {}",
                    string_key(req.key()),
                    req.flags(),
                    ttl,
                    req.value().len(),
                    code,
                    len
                );
            }
            Request::Add(ref req) => {
                let ttl: i64 = match req.ttl() {
                    None => 0,
                    Some(0) => -1,
                    Some(t) => t as _,
                };
                let (code, len) = match self.response {
                    Response::Stored(ref res) => {
                        ADD_STORED.increment();
                        (STORED, res.len())
                    }
                    Response::NotStored(ref res) => {
                        ADD_NOT_STORED.increment();
                        (NOT_STORED, res.len())
                    }
                    _ => return Error {}.compose(dst),
                };
                klog!(
                    "\"add {} {} {} {}\" {} {}",
                    string_key(req.key()),
                    req.flags(),
                    ttl,
                    req.value().len(),
                    code,
                    len
                );
            }
            Request::Replace(ref req) => {
                let ttl: i64 = match req.ttl() {
                    None => 0,
                    Some(0) => -1,
                    Some(t) => t as _,
                };
                let (code, len) = match self.response {
                    Response::Stored(ref res) => {
                        REPLACE_STORED.increment();
                        (STORED, res.len())
                    }
                    Response::NotStored(ref res) => {
                        REPLACE_NOT_STORED.increment();
                        (NOT_STORED, res.len())
                    }
                    _ => return Error {}.compose(dst),
                };
                klog!(
                    "\"replace {} {} {} {}\" {} {}",
                    string_key(req.key()),
                    req.flags(),
                    ttl,
                    req.value().len(),
                    code,
                    len
                );
            }
            Request::Cas(ref req) => {
                let ttl: i64 = match req.ttl() {
                    None => 0,
                    Some(0) => -1,
                    Some(t) => t as _,
                };
                let (code, len) = match self.response {
                    Response::Stored(ref res) => {
                        CAS_STORED.increment();
                        (STORED, res.len())
                    }
                    Response::Exists(ref res) => {
                        CAS_EXISTS.increment();
                        (EXISTS, res.len())
                    }
                    Response::NotFound(ref res) => {
                        CAS_NOT_FOUND.increment();
                        (NOT_FOUND, res.len())
                    }
                    _ => return Error {}.compose(dst),
                };
                klog!(
                    "\"cas {} {} {} {} {}\" {} {}",
                    string_key(req.key()),
                    req.flags(),
                    ttl,
                    req.value().len(),
                    req.cas(),
                    code,
                    len
                );
            }
            Request::Append(ref req) => {
                let ttl: i64 = match req.ttl() {
                    None => 0,
                    Some(0) => -1,
                    Some(t) => t as _,
                };
                let (code, len) = match self.response {
                    Response::Stored(ref res) => {
                        APPEND_STORED.increment();
                        (STORED, res.len())
                    }
                    Response::NotStored(ref res) => {
                        APPEND_NOT_STORED.increment();
                        (NOT_STORED, res.len())
                    }
                    _ => return Error {}.compose(dst),
                };
                klog!(
                    "\"append {} {} {} {}\" {} {}",
                    string_key(req.key()),
                    req.flags(),
                    ttl,
                    req.value().len(),
                    code,
                    len
                );
            }
            Request::Prepend(ref req) => {
                let ttl: i64 = match req.ttl() {
                    None => 0,
                    Some(0) => -1,
                    Some(t) => t as _,
                };
                let (code, len) = match self.response {
                    Response::Stored(ref res) => {
                        PREPEND_STORED.increment();
                        (STORED, res.len())
                    }
                    Response::NotStored(ref res) => {
                        PREPEND_NOT_STORED.increment();
                        (NOT_STORED, res.len())
                    }
                    _ => return Error {}.compose(dst),
                };
                klog!(
                    "\"prepend {} {} {} {}\" {} {}",
                    string_key(req.key()),
                    req.flags(),
                    ttl,
                    req.value().len(),
                    code,
                    len
                );
            }
            Request::Incr(ref req) => {
                let (code, len) = match self.response {
                    Response::Numeric(ref res) => {
                        INCR_STORED.increment();
                        (STORED, res.len())
                    }
                    Response::NotFound(ref res) => {
                        INCR_NOT_FOUND.increment();
                        (NOT_FOUND, res.len())
                    }
                    _ => return Error {}.compose(dst),
                };
                klog!("\"incr {}\" {} {}", string_key(req.key()), code, len);
            }
            Request::Decr(ref req) => {
                let (code, len) = match self.response {
                    Response::Numeric(ref res) => {
                        DECR_STORED.increment();
                        (STORED, res.len())
                    }
                    Response::NotFound(ref res) => {
                        DECR_NOT_FOUND.increment();
                        (NOT_FOUND, res.len())
                    }
                    _ => return Error {}.compose(dst),
                };
                klog!("\"decr {}\" {} {}", string_key(req.key()), code, len);
            }
            Request::Delete(ref req) => {
                let (code, len) = match self.response {
                    Response::Deleted(ref res) => {
                        DELETE_DELETED.increment();
                        (DELETED, res.len())
                    }
                    Response::NotFound(ref res) => {
                        DELETE_NOT_FOUND.increment();
                        (NOT_FOUND, res.len())
                    }
                    _ => return Error {}.compose(dst),
                };
                klog!("\"delete {}\" {} {}", string_key(req.key()), code, len);
            }
            Request::FlushAll(_) => {}
            Request::Quit(_) => {}
        }
        self.response.compose(dst)
    }
}

fn string_key(key: &[u8]) -> Cow<'_, str> {
    String::from_utf8_lossy(key)
}
