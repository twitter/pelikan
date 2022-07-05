// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Encodes the client side of the memcache service, where we send requests and
//! parse responses.

use rustcommon_metrics::*;
use service_common::*;

use protocol_common::*;
use protocol_memcache::*;
use session::Session;

client_counter!(COMPOSE_GET, "compose/get");
client_counter!(PARSE_GET_ERROR, "parse/get/error");
client_counter!(PARSE_GET_KEY_HIT, "parse/get/key_hit");
client_counter!(PARSE_GET_KEY_MISS, "parse/get/key_miss");

client_counter!(COMPOSE_GETS, "compose/gets");
client_counter!(PARSE_GETS_ERROR, "parse/gets/error");
client_counter!(PARSE_GETS_KEY_HIT, "parse/gets/key_hit");
client_counter!(PARSE_GETS_KEY_MISS, "parse/gets/key_miss");

client_counter!(COMPOSE_SET, "compose/set");
client_counter!(PARSE_SET_ERROR, "parse/set/error");
client_counter!(PARSE_SET_STORED, "parse/set/stored");
client_counter!(PARSE_SET_NOT_STORED, "parse/set/not_stored");

client_counter!(COMPOSE_ADD, "compose/add");
client_counter!(PARSE_ADD_ERROR, "parse/add/error");
client_counter!(PARSE_ADD_STORED, "parse/add/stored");
client_counter!(PARSE_ADD_NOT_STORED, "parse/add/not_stored");

client_counter!(COMPOSE_REPLACE, "compose/replace");
client_counter!(PARSE_REPLACE_ERROR, "parse/replace/error");
client_counter!(PARSE_REPLACE_STORED, "parse/replace/stored");
client_counter!(PARSE_REPLACE_NOT_STORED, "parse/replace/not_stored");

client_counter!(COMPOSE_APPEND, "compose/append");
client_counter!(PARSE_APPEND_ERROR, "parse/append/error");
client_counter!(PARSE_APPEND_STORED, "parse/append/stored");
client_counter!(PARSE_APPEND_NOT_STORED, "parse/append/not_stored");

client_counter!(COMPOSE_PREPEND, "compose/prepend");
client_counter!(PARSE_PREPEND_ERROR, "parse/prepend/error");
client_counter!(PARSE_PREPEND_STORED, "parse/prepend/stored");
client_counter!(PARSE_PREPEND_NOT_STORED, "parse/prepend/not_stored");

client_counter!(COMPOSE_INCR, "compose/incr");
client_counter!(PARSE_INCR_ERROR, "parse/incr/error");
client_counter!(PARSE_INCR_NOT_FOUND, "parse/incr/not_found");

client_counter!(COMPOSE_DECR, "compose/decr");
client_counter!(PARSE_DECR_ERROR, "parse/decr/error");
client_counter!(PARSE_DECR_NOT_FOUND, "parse/decr/not_found");

client_counter!(COMPOSE_DELETE, "compose/delete");
client_counter!(PARSE_DELETE_ERROR, "parse/delete/error");
client_counter!(PARSE_DELETE_DELETED, "parse/delete/deleted");
client_counter!(PARSE_DELETE_NOT_FOUND, "parse/delete/not_found");

client_counter!(COMPOSE_CAS, "compose/cas");
client_counter!(PARSE_CAS_ERROR, "parse/cas/error");
client_counter!(PARSE_CAS_STORED, "parse/cas/stored");
client_counter!(PARSE_CAS_EXISTS, "parse/cas/exists");
client_counter!(PARSE_CAS_NOT_FOUND, "parse/cas/not_found");

client_counter!(COMPOSE_FLUSH_ALL, "compose/flush_all");
client_counter!(PARSE_FLUSH_ALL_ERROR, "parse/flush_all/error");
client_counter!(PARSE_FLUSH_ALL_OK, "parse/flush_all/ok");

client_counter!(COMPOSE_QUIT, "compose/quit");

client_counter!(PARSE_INVALID, "parse/invalid");

pub struct MemcacheClient {
    parser: ResponseParser,
}

impl From<ResponseParser> for MemcacheClient {
    fn from(other: ResponseParser) -> Self {
        Self { parser: other }
    }
}

impl Client<Request, Response> for MemcacheClient {
    fn send(&self, dst: &mut Session, req: &Request) {
        match req {
            Request::Get(_) => COMPOSE_GET.increment(),
            Request::Gets(_) => COMPOSE_GETS.increment(),
            Request::Set(_) => COMPOSE_SET.increment(),
            Request::Add(_) => COMPOSE_ADD.increment(),
            Request::Replace(_) => COMPOSE_REPLACE.increment(),
            Request::Cas(_) => COMPOSE_CAS.increment(),
            Request::Append(_) => COMPOSE_APPEND.increment(),
            Request::Prepend(_) => COMPOSE_PREPEND.increment(),
            Request::Incr(_) => COMPOSE_INCR.increment(),
            Request::Decr(_) => COMPOSE_DECR.increment(),
            Request::Delete(_) => COMPOSE_DELETE.increment(),
            Request::FlushAll(_) => COMPOSE_FLUSH_ALL.increment(),
            Request::Quit(_) => COMPOSE_QUIT.increment(),
        };
        req.compose(dst)
    }

    fn recv(&self, src: &[u8], req: &Request) -> Result<ParseOk<Response>, ParseError> {
        let res = self.parser.parse(src)?;

        let consumed = res.consumed();
        let res = res.into_inner();

        match req {
            Request::Get(ref req) => match res {
                // get requests respond with a `values` response or some error
                Response::Values(ref res) => {
                    let total_keys = req.keys().len();
                    let hit_keys = res.values().len();
                    let miss_keys = total_keys - hit_keys;
                    PARSE_GET_KEY_HIT.add(hit_keys as _);
                    PARSE_GET_KEY_MISS.add(miss_keys as _);
                }
                Response::ClientError(_) | Response::ServerError(_) | Response::Error(_) => {
                    PARSE_GET_ERROR.increment();
                }
                _ => {
                    // transform an unexpected response into an error
                    PARSE_INVALID.increment();
                    return Err(ParseError::Invalid);
                }
            },
            Request::Gets(ref req) => match res {
                // gets requests respond with a `values` response or some error
                Response::Values(ref res) => {
                    let total_keys = req.keys().len();
                    let hit_keys = res.values().len();
                    let miss_keys = total_keys - hit_keys;
                    PARSE_GETS_KEY_HIT.add(hit_keys as _);
                    PARSE_GETS_KEY_MISS.add(miss_keys as _);
                }
                Response::ClientError(_) | Response::ServerError(_) | Response::Error(_) => {
                    PARSE_GETS_ERROR.increment();
                }
                _ => {
                    // transform an unexpected response into an error
                    PARSE_INVALID.increment();
                    return Err(ParseError::Invalid);
                }
            },
            Request::Set(_) => match res {
                Response::Stored(_) => {
                    PARSE_SET_STORED.increment();
                }
                Response::NotStored(_) => {
                    PARSE_SET_NOT_STORED.increment();
                }
                Response::ClientError(_) | Response::ServerError(_) | Response::Error(_) => {
                    PARSE_SET_ERROR.increment();
                }
                _ => {
                    // transform an unexpected response into an error
                    PARSE_INVALID.increment();
                    return Err(ParseError::Invalid);
                }
            },
            Request::Add(_) => match res {
                Response::Stored(_) => {
                    PARSE_ADD_STORED.increment();
                }
                Response::NotStored(_) => {
                    PARSE_ADD_NOT_STORED.increment();
                }
                Response::ClientError(_) | Response::ServerError(_) | Response::Error(_) => {
                    PARSE_ADD_ERROR.increment();
                }
                _ => {
                    // transform an unexpected response into an error
                    PARSE_INVALID.increment();
                    return Err(ParseError::Invalid);
                }
            },
            Request::Replace(_) => match res {
                Response::Stored(_) => {
                    PARSE_REPLACE_STORED.increment();
                }
                Response::NotStored(_) => {
                    PARSE_REPLACE_NOT_STORED.increment();
                }
                Response::ClientError(_) | Response::ServerError(_) | Response::Error(_) => {
                    PARSE_REPLACE_ERROR.increment();
                }
                _ => {
                    // transform an unexpected response into an error
                    PARSE_INVALID.increment();
                    return Err(ParseError::Invalid);
                }
            },
            Request::Cas(_) => match res {
                Response::Stored(_) => {
                    PARSE_CAS_STORED.increment();
                }
                Response::Exists(_) => {
                    PARSE_CAS_EXISTS.increment();
                }
                Response::NotFound(_) => {
                    PARSE_CAS_NOT_FOUND.increment();
                }
                Response::ClientError(_) | Response::ServerError(_) | Response::Error(_) => {
                    PARSE_CAS_ERROR.increment();
                }
                _ => {
                    // transform an unexpected response into an error
                    PARSE_INVALID.increment();
                    return Err(ParseError::Invalid);
                }
            },
            Request::Append(_) => match res {
                Response::Stored(_) => {
                    PARSE_APPEND_STORED.increment();
                }
                Response::NotStored(_) => {
                    PARSE_APPEND_NOT_STORED.increment();
                }
                Response::ClientError(_) | Response::ServerError(_) | Response::Error(_) => {
                    PARSE_APPEND_ERROR.increment();
                }
                _ => {
                    // transform an unexpected response into an error
                    PARSE_INVALID.increment();
                    return Err(ParseError::Invalid);
                }
            },
            Request::Prepend(_) => match res {
                Response::Stored(_) => {
                    PARSE_PREPEND_STORED.increment();
                }
                Response::NotStored(_) => {
                    PARSE_PREPEND_NOT_STORED.increment();
                }
                Response::ClientError(_) | Response::ServerError(_) | Response::Error(_) => {
                    PARSE_PREPEND_ERROR.increment();
                }
                _ => {
                    // transform an unexpected response into an error
                    PARSE_INVALID.increment();
                    return Err(ParseError::Invalid);
                }
            },
            Request::Incr(_) => match res {
                Response::Numeric(_) => {}
                Response::NotFound(_) => {
                    PARSE_INCR_NOT_FOUND.increment();
                }
                Response::ClientError(_) | Response::ServerError(_) | Response::Error(_) => {
                    PARSE_INCR_ERROR.increment();
                }
                _ => {
                    // transform an unexpected response into an error
                    PARSE_INVALID.increment();
                    return Err(ParseError::Invalid);
                }
            },
            Request::Decr(_) => match res {
                Response::Numeric(_) => {}
                Response::NotFound(_) => {
                    PARSE_DECR_NOT_FOUND.increment();
                }
                Response::ClientError(_) | Response::ServerError(_) | Response::Error(_) => {
                    PARSE_DECR_ERROR.increment();
                }
                _ => {
                    // transform an unexpected response into an error
                    PARSE_INVALID.increment();
                    return Err(ParseError::Invalid);
                }
            },
            Request::Delete(_) => match res {
                Response::Deleted(_) => {
                    PARSE_DELETE_DELETED.increment();
                }
                Response::NotFound(_) => {
                    PARSE_DELETE_NOT_FOUND.increment();
                }
                Response::ClientError(_) | Response::ServerError(_) | Response::Error(_) => {
                    PARSE_DELETE_ERROR.increment();
                }
                _ => {
                    // transform an unexpected response into an error
                    PARSE_INVALID.increment();
                    return Err(ParseError::Invalid);
                }
            },
            Request::FlushAll(_) => match res {
                Response::Ok(_) => {
                    PARSE_FLUSH_ALL_OK.increment();
                }
                Response::ClientError(_) | Response::ServerError(_) | Response::Error(_) => {
                    PARSE_FLUSH_ALL_ERROR.increment();
                }
                _ => {
                    // transform an unexpected response into an error
                    PARSE_INVALID.increment();
                    return Err(ParseError::Invalid);
                }
            },
            Request::Quit(_) => {}
        }

        Ok(ParseOk::new(res, consumed))
    }
}
