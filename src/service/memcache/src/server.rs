// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use super::*;

use protocol_common::*;
use protocol_memcache::*;
use session::Session;

server_counter!(PARSE_GET, "parse/get");
server_counter!(PARSE_GET_EX, "parse/get_ex");
server_counter!(COMPOSE_GET_KEY_HIT, "compose/get/key_hit");
server_counter!(COMPOSE_GET_KEY_MISS, "compose/get/key_miss");

server_counter!(PARSE_SET, "parse/set");
server_counter!(PARSE_SET_EX, "parse/set_ex");
server_counter!(COMPOSE_SET_STORED, "compose/set/stored");
server_counter!(COMPOSE_SET_NOT_STORED, "compose/set/not_stored");

pub struct Server {
	parser: RequestParser,
}

impl Server {
	pub fn recv(&self, src: &[u8]) -> Result<ParseOk<Request>, ParseError> {
		self.parser.parse(src)
	}

	pub fn send(&self, dst: &mut Session, req: Request, res: Response) {
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
            Request::Set(_) => match res {
                Response::Stored(_) => {
                    COMPOSE_SET_STORED.increment();
                }
                Response::NotStored(_) => {
                    COMPOSE_SET_NOT_STORED.increment();
                }
                _ => return Error {}.compose(dst),
            },
            _ => {
            	todo!()
            }
		}
		res.compose(dst)
	}
}
