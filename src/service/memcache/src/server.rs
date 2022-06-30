// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

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
server_counter!(COMPOSE_INCR_NOT_FOUND, "compose/incr/not_found");

server_counter!(PARSE_DECR, "parse/decr");
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
        match req {
            Request::Get(ref req) => match res {
                Response::Values(ref res) => {
                    let total_keys = req.keys().len();
                    let hit_keys = res.values().len();
                    let miss_keys = total_keys - hit_keys;
                    COMPOSE_GET_KEY_HIT.add(hit_keys as _);
                    COMPOSE_GET_KEY_MISS.add(miss_keys as _);
                }
                _ => return Error {}.compose(dst),
            },
            Request::Gets(ref req) => match res {
                Response::Values(ref res) => {
                    let total_keys = req.keys().len();
                    let hit_keys = res.values().len();
                    let miss_keys = total_keys - hit_keys;
                    COMPOSE_GETS_KEY_HIT.add(hit_keys as _);
                    COMPOSE_GETS_KEY_MISS.add(miss_keys as _);
                }
                _ => return Error {}.compose(dst),
            },
            Request::Set(_) => match res {
                Response::Stored(_) => {
                    COMPOSE_SET_STORED.increment();
                }
                Response::NotStored(_) => {
                    COMPOSE_SET_NOT_STORED.increment();
                }
                _ => return Error {}.compose(dst),
            },
            Request::Add(_) => match res {
                Response::Stored(_) => {
                    COMPOSE_ADD_STORED.increment();
                }
                Response::NotStored(_) => {
                    COMPOSE_ADD_NOT_STORED.increment();
                }
                _ => return Error {}.compose(dst),
            },
            Request::Replace(_) => match res {
                Response::Stored(_) => {
                    COMPOSE_REPLACE_STORED.increment();
                }
                Response::NotStored(_) => {
                    COMPOSE_REPLACE_NOT_STORED.increment();
                }
                _ => return Error {}.compose(dst),
            },
            Request::Cas(_) => match res {
                Response::Stored(_) => {
                    COMPOSE_CAS_STORED.increment();
                }
                Response::Exists(_) => {
                    COMPOSE_CAS_EXISTS.increment();
                }
                Response::NotFound(_) => {
                    COMPOSE_CAS_NOT_FOUND.increment();
                }
                _ => return Error {}.compose(dst),
            },
            Request::Append(_) => match res {
                Response::Stored(_) => {
                    COMPOSE_APPEND_STORED.increment();
                }
                Response::NotStored(_) => {
                    COMPOSE_APPEND_NOT_STORED.increment();
                }
                _ => return Error {}.compose(dst),
            },
            Request::Prepend(_) => match res {
                Response::Stored(_) => {
                    COMPOSE_PREPEND_STORED.increment();
                }
                Response::NotStored(_) => {
                    COMPOSE_PREPEND_NOT_STORED.increment();
                }
                _ => return Error {}.compose(dst),
            },
            Request::Incr(_) => match res {
                Response::Numeric(_) => {
                    // do nothing, we don't know that the value changed
                },
                Response::NotFound(_) => {
                    COMPOSE_INCR_NOT_FOUND.increment();
                }
                _ => return Error {}.compose(dst),
            },
            Request::Decr(_) => match res {
                Response::Numeric(_) => {
                    // do nothing, we don't know that the value changed
                },
                Response::NotFound(_) => {
                    COMPOSE_DECR_NOT_FOUND.increment();
                }
                _ => return Error {}.compose(dst),
            },
            Request::Delete(_) => match res {
                Response::Deleted(_) => {
                    COMPOSE_DELETE_DELETED.increment();
                }
                Response::NotFound(_) => {
                    COMPOSE_DELETE_NOT_FOUND.increment();
                }
                _ => return Error {}.compose(dst),
            }
            Request::FlushAll(_) => match res {
                Response::Ok(_) => {
                    COMPOSE_FLUSH_ALL_OK.increment();
                }
                _ => return Error {}.compose(dst),
            }
            Request::Quit(_) => {},
            
            
        }
        res.compose(dst)
    }
}
