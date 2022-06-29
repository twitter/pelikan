// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use super::*;

use protocol_common::*;
use protocol_memcache::*;
use session::Session;

client_counter!(COMPOSE_GET, "compose/get");
client_counter!(PARSE_GET_ERROR, "parse/get/error");
client_counter!(PARSE_GET_KEY_HIT, "parse/get/key_hit");
client_counter!(PARSE_GET_KEY_MISS, "parse/get/key_miss");

client_counter!(COMPOSE_SET, "compose/set");
client_counter!(PARSE_SET_ERROR, "parse/set/error");
client_counter!(PARSE_SET_STORED, "parse/set/stored");
client_counter!(PARSE_SET_NOT_STORED, "parse/set/not_stored");

client_counter!(PARSE_INVALID, "parse/invalid");

pub struct Client {
    parser: ResponseParser,
}

impl Client {
    pub fn send(&self, dst: &mut Session, req: &Request) {
        req.compose(dst)
    }

    pub fn recv(&self, src: &[u8], req: &Request) -> Result<ParseOk<Response>, ParseError> {
        let res = self.parser.parse(src)?;

        let consumed = res.consumed();
        let res = res.into_inner();

        match req {
            Request::Get(ref req) => match res {
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
            _ => todo!(),
        }

        Ok(ParseOk::new(res, consumed))
    }
}
