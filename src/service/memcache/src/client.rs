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
client_counter!(PARSE_INCR_STORED, "parse/incr/stored");
client_counter!(PARSE_INCR_ERROR, "parse/incr/error");
client_counter!(PARSE_INCR_NOT_FOUND, "parse/incr/not_found");

client_counter!(COMPOSE_DECR, "compose/decr");
client_counter!(PARSE_DECR_ERROR, "parse/decr/error");
client_counter!(PARSE_DECR_STORED, "parse/decr/stored");
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

        match (req, &res) {
            // get
            (Request::Get(ref req), Response::Values(ref res)) => {
                let total_keys = req.keys().len();
                let hit_keys = res.values().len();
                let miss_keys = total_keys - hit_keys;
                PARSE_GET_KEY_HIT.add(hit_keys as _);
                PARSE_GET_KEY_MISS.add(miss_keys as _);
            }
            (Request::Get(_), response) => {
                handle_error(&PARSE_GET_ERROR, response)?;
            }
            // gets
            (Request::Gets(ref req), Response::Values(ref res)) => {
                let total_keys = req.keys().len();
                let hit_keys = res.values().len();
                let miss_keys = total_keys - hit_keys;
                PARSE_GETS_KEY_HIT.add(hit_keys as _);
                PARSE_GETS_KEY_MISS.add(miss_keys as _);
            }
            (Request::Gets(_), response) => {
                handle_error(&PARSE_GETS_ERROR, response)?;
            }
            // set
            (Request::Set(_), Response::Stored(_)) => {
                PARSE_SET_STORED.increment();
            }
            (Request::Set(_), Response::NotStored(_)) => {
                PARSE_SET_NOT_STORED.increment();
            }
            (Request::Set(_), response) => {
                handle_error(&PARSE_SET_ERROR, response)?;
            }
            // add
            (Request::Add(_), Response::Stored(_)) => {
                PARSE_ADD_STORED.increment();
            }
            (Request::Add(_), Response::NotStored(_)) => {
                PARSE_ADD_NOT_STORED.increment();
            }
            (Request::Add(_), response) => {
                handle_error(&PARSE_ADD_ERROR, response)?;
            }
            // replace
            (Request::Replace(_), Response::Stored(_)) => {
                PARSE_REPLACE_STORED.increment();
            }
            (Request::Replace(_), Response::NotStored(_)) => {
                PARSE_REPLACE_NOT_STORED.increment();
            }
            (Request::Replace(_), response) => {
                handle_error(&PARSE_REPLACE_ERROR, response)?;
            }
            // cas
            (Request::Cas(_), Response::Stored(_)) => {
                PARSE_CAS_STORED.increment();
            }
            (Request::Cas(_), Response::Exists(_)) => {
                PARSE_CAS_EXISTS.increment();
            }
            (Request::Cas(_), Response::NotFound(_)) => {
                PARSE_CAS_NOT_FOUND.increment();
            }
            (Request::Cas(_), response) => {
                handle_error(&PARSE_CAS_ERROR, response)?;
            }
            // append
            (Request::Append(_), Response::Stored(_)) => {
                PARSE_APPEND_STORED.increment();
            }
            (Request::Append(_), Response::NotStored(_)) => {
                PARSE_APPEND_NOT_STORED.increment();
            }
            (Request::Append(_), response) => {
                handle_error(&PARSE_APPEND_ERROR, response)?;
            }
            // prepend
            (Request::Prepend(_), Response::Stored(_)) => {
                PARSE_PREPEND_STORED.increment();
            }
            (Request::Prepend(_), Response::NotStored(_)) => {
                PARSE_PREPEND_NOT_STORED.increment();
            }
            (Request::Prepend(_), response) => {
                handle_error(&PARSE_PREPEND_ERROR, response)?;
            }
            // incr
            (Request::Incr(_), Response::Numeric(_)) => {
                PARSE_INCR_STORED.increment();
            }
            (Request::Incr(_), Response::NotFound(_)) => {
                PARSE_INCR_NOT_FOUND.increment();
            }
            (Request::Incr(_), response) => {
                handle_error(&PARSE_INCR_ERROR, response)?;
            }
            // decr
            (Request::Decr(_), Response::Numeric(_)) => {
                PARSE_DECR_STORED.increment();
            }
            (Request::Decr(_), Response::NotFound(_)) => {
                PARSE_DECR_NOT_FOUND.increment();
            }
            (Request::Decr(_), response) => {
                handle_error(&PARSE_INCR_ERROR, response)?;
            }
            // delete
            (Request::Delete(_), Response::Deleted(_)) => {
                PARSE_DELETE_DELETED.increment();
            }
            (Request::Delete(_), Response::NotFound(_)) => {
                PARSE_DELETE_NOT_FOUND.increment();
            }
            (Request::Delete(_), response) => {
                handle_error(&PARSE_DELETE_ERROR, response)?;
            }
            // flush_all
            (Request::FlushAll(_), Response::Ok(_)) => {
                PARSE_FLUSH_ALL_OK.increment();
            }
            (Request::FlushAll(_), response) => {
                handle_error(&PARSE_FLUSH_ALL_ERROR, response)?;
            }
            (Request::Quit(_), _) => {}
        }

        Ok(ParseOk::new(res, consumed))
    }
}

fn handle_error(counter: &'static Counter, response: &Response) -> Result<(), ParseError> {
    if response.is_error() {
        counter.increment();
        Ok(())
    } else {
        // transform an unexpected response into an error
        PARSE_INVALID.increment();
        Err(ParseError::Invalid)
    }
}
