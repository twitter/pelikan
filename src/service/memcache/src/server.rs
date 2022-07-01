// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use core::fmt::Display;
use std::io::Write;
use std::ops::Deref;
use service_common::*;
use rustcommon_metrics::*;

use protocol_common::*;
use protocol_memcache::*;
use session::Session;

server_counter!(PARSE_GET, "parse/get");
server_counter!(PARSE_GET_KEY, "parse/get_key");
server_counter!(COMPOSE_GET_KEY_HIT, "compose/get/key_hit");
server_counter!(COMPOSE_GET_KEY_MISS, "compose/get/key_miss");

server_counter!(PARSE_GETS, "parse/gets");
server_counter!(PARSE_GETS_KEY, "parse/gets_key");
server_counter!(COMPOSE_GETS_KEY_HIT, "compose/gets/key_hit");
server_counter!(COMPOSE_GETS_KEY_MISS, "compose/gets/key_miss");

server_counter!(PARSE_SET, "parse/set");
server_counter!(COMPOSE_SET_STORED, "compose/set/stored");
server_counter!(COMPOSE_SET_NOT_STORED, "compose/set/not_stored");

server_counter!(PARSE_ADD, "parse/add");
server_counter!(COMPOSE_ADD_STORED, "compose/add/stored");
server_counter!(COMPOSE_ADD_NOT_STORED, "compose/add/not_stored");

server_counter!(PARSE_REPLACE, "parse/replace");
server_counter!(COMPOSE_REPLACE_STORED, "compose/replace/stored");
server_counter!(COMPOSE_REPLACE_NOT_STORED, "compose/replace/not_stored");

server_counter!(PARSE_CAS, "parse/cas");
server_counter!(COMPOSE_CAS_STORED, "compose/cas/stored");
server_counter!(COMPOSE_CAS_EXISTS, "compose/cas/exists");
server_counter!(COMPOSE_CAS_NOT_FOUND, "compose/cas/not_found");

server_counter!(PARSE_APPEND, "parse/append");
server_counter!(COMPOSE_APPEND_STORED, "compose/append/stored");
server_counter!(COMPOSE_APPEND_NOT_STORED, "compose/append/not_stored");

server_counter!(PARSE_PREPEND, "parse/prepend");
server_counter!(COMPOSE_PREPEND_STORED, "compose/prepend/stored");
server_counter!(COMPOSE_PREPEND_NOT_STORED, "compose/prepend/not_stored");

server_counter!(PARSE_INCR, "parse/incr");
server_counter!(COMPOSE_INCR_STORED, "compose/incr/stored");
server_counter!(COMPOSE_INCR_NOT_FOUND, "compose/incr/not_found");

server_counter!(PARSE_DECR, "parse/decr");
server_counter!(COMPOSE_DECR_STORED, "compose/decr/stored");
server_counter!(COMPOSE_DECR_NOT_FOUND, "compose/decr/not_found");

server_counter!(PARSE_DELETE, "parse/delete");
server_counter!(COMPOSE_DELETE_DELETED, "compose/delete/deleted");
server_counter!(COMPOSE_DELETE_NOT_FOUND, "compose/delete/not_found");

server_counter!(PARSE_FLUSH_ALL, "parse/flush_all");
server_counter!(COMPOSE_FLUSH_ALL_OK, "compose/flush_all/ok");

server_counter!(PARSE_QUIT, "parse/quit");

#[derive(Clone)]
pub struct MemcacheServer {
    parser: RequestParser,
}

impl From<RequestParser> for MemcacheServer {
    fn from(other: RequestParser) -> Self {
        Self {
            parser: other,
        }
    }
}

impl Server<Request, Response> for MemcacheServer {
    fn recv(&self, src: &[u8]) -> Result<ParseOk<Request>, ParseError> {
        let req = self.parser.parse(src)?;

        let consumed = req.consumed();
        let req = req.into_inner();

        match req {
            Request::Get(ref req) => {
                let keys = req.keys().len();

                PARSE_GET_KEY.add(keys as _)
            }
            Request::Gets(ref req) => {
                let keys = req.keys().len();

                PARSE_GETS_KEY.add(keys as _)
            }
            Request::Set(_) => PARSE_SET.increment(),
            Request::Add(_) => PARSE_ADD.increment(),
            Request::Replace(_) => PARSE_REPLACE.increment(),
            Request::Cas(_) => PARSE_CAS.increment(),
            Request::Append(_) => PARSE_APPEND.increment(),
            Request::Prepend(_) => PARSE_PREPEND.increment(),
            Request::Incr(_) => PARSE_INCR.increment(),
            Request::Decr(_) => PARSE_DECR.increment(),
            Request::Delete(_) => PARSE_DELETE.increment(),
            Request::FlushAll(_) => PARSE_FLUSH_ALL.increment(),
            Request::Quit(_) => PARSE_QUIT.increment(),
        };

        Ok(ParseOk::new(req, consumed))
    }

    fn send(&self, dst: &mut Session, req: Request, res: Response) {
        match (req, res) {
            (Request::Get(req), Response::Values(res)) => {
                let (hit, miss) = compose_read(dst, &req, &res);
                COMPOSE_GET_KEY_HIT.add(hit as _);
                COMPOSE_GET_KEY_MISS.add(miss as _);
            }
            (Request::Gets(req), Response::Values(res)) => {
                let (hit, miss) = compose_read(dst, &req, &res);
                COMPOSE_GET_KEY_HIT.add(hit as _);
                COMPOSE_GET_KEY_MISS.add(miss as _);
            }
            (Request::Set(req), Response::Stored(res)) => {
                compose_write(dst, &req, &res);
                COMPOSE_SET_STORED.increment();
            }
            (Request::Set(req), Response::NotStored(res)) => {
                compose_write(dst, &req, &res);
                COMPOSE_SET_NOT_STORED.increment();
            }
            (Request::Add(req), Response::Stored(res)) => {
                compose_write(dst, &req, &res);
                COMPOSE_ADD_STORED.increment();
            }
            (Request::Add(req), Response::NotStored(res)) => {
                compose_write(dst, &req, &res);
                COMPOSE_ADD_NOT_STORED.increment();
            }
            (Request::Replace(req), Response::Stored(res)) => {
                compose_write(dst, &req, &res);
                COMPOSE_REPLACE_STORED.increment();
            }
            (Request::Replace(req), Response::NotStored(res)) => {
                compose_write(dst, &req, &res);
                COMPOSE_REPLACE_NOT_STORED.increment();
            }
            (Request::Cas(req), Response::Stored(res)) => {
                compose_cas(dst, &req, &res);
                COMPOSE_CAS_STORED.increment();
            }
            (Request::Cas(req), Response::Exists(res)) => {
                compose_cas(dst, &req, &res);
                COMPOSE_CAS_EXISTS.increment();
            }
            (Request::Cas(req), Response::NotFound(res)) => {
                compose_cas(dst, &req, &res);
                COMPOSE_CAS_NOT_FOUND.increment();
            }
            (Request::Append(req), Response::Stored(res)) => {
                compose_write(dst, &req, &res);
                COMPOSE_APPEND_STORED.increment();
            }
            (Request::Append(req), Response::NotStored(res)) => {
                compose_write(dst, &req, &res);
                COMPOSE_APPEND_NOT_STORED.increment();
            }
            (Request::Prepend(req), Response::Stored(res)) => {
                compose_write(dst, &req, &res);
                COMPOSE_PREPEND_STORED.increment();
            }
            (Request::Prepend(req), Response::NotStored(res)) => {
                compose_write(dst, &req, &res);
                COMPOSE_PREPEND_NOT_STORED.increment();
            }
            (Request::Incr(req), Response::Stored(res)) => {
                compose_modify(dst, &req, &res);
                COMPOSE_INCR_STORED.increment();
            }
            (Request::Incr(req), Response::NotFound(res)) => {
                compose_modify(dst, &req, &res);
                COMPOSE_INCR_NOT_FOUND.increment();
            }
            (Request::Decr(req), Response::Stored(res)) => {
                compose_modify(dst, &req, &res);
                COMPOSE_DECR_STORED.increment();
            }
            (Request::Decr(req), Response::NotFound(res)) => {
                compose_modify(dst, &req, &res);
                COMPOSE_DECR_NOT_FOUND.increment();
            }
            (Request::Delete(req), Response::Deleted(res)) => {
                compose_modify(dst, &req, &res);
                COMPOSE_DELETE_DELETED.increment();
            }
            (Request::Delete(req), Response::NotFound(res)) => {
                compose_modify(dst, &req, &res);
                COMPOSE_DELETE_NOT_FOUND.increment();
            }
            (Request::FlushAll(_), Response::Ok(res)) => {
                // this isn't command logged
                let _ = res.compose(dst);
                COMPOSE_FLUSH_ALL_OK.increment();
            }
            (Request::Quit(_), _) => {},
            _ => {
                // We have some unexpected request, respomse, or
                // request-response pair. This is treated as an internal error.
                let _ = Error::default().compose(dst);
            }
        }
    }
}


fn compose_read<T: Display + Keys>(dst: &mut Session, req: &T, res: &Values) -> (usize, usize) {
    let total_keys = req.keys().len();
    let hit_keys = res.values().len();
    let miss_keys = total_keys - hit_keys;

    let values = res.values();
    let mut value_index = 0;

    for key in req.keys() {
        let key = key.deref();
        // if we are out of values or the keys don't match, it's a miss
        if value_index >= values.len() || values[value_index].key() != key {
            klog!("\"{} {}\" 0 0", req, String::from_utf8_lossy(key));
        } else {
            let start = dst.write_pending();
            values[value_index].compose(dst);
            let size = dst.write_pending() - start;
            klog!("\"{} {}\" 4 {}", req, String::from_utf8_lossy(key), size);
            value_index += 1;
        }
    }

    let _ = dst.write_all(b"END\r\n");

    (hit_keys, miss_keys)
}

fn compose_modify<T: Display + Key, U: Compose + SimpleResponse>(dst: &mut Session, req: &T, res: &U) {
    klog!("\"{} {}\" {} {}",
        req,
        req.key_as_str(),
        res.code(),
        res.len(),
    );

    res.compose(dst);
}

fn compose_write<T: Display + Key + Ttl + RequestValue + Flags, U: Compose + SimpleResponse>(dst: &mut Session, req: &T, res: &U) {
    klog!("\"{} {} {} {} {}\" {} {}",
        req,
        req.key_as_str(),
        req.flags(),
        req.ttl_as_i64(),
        req.value().len(),
        res.code(),
        res.len(),
    );

    res.compose(dst);
}

fn compose_cas<U: Compose + SimpleResponse>(dst: &mut Session, req: &Cas, res: &U) {
    klog!("\"{} {} {} {} {} {}\" {} {}",
        req,
        req.key_as_str(),
        req.flags(),
        req.ttl_as_i64(),
        req.value().len(),
        req.cas(),
        res.code(),
        res.len(),
    );

    res.compose(dst);
}
