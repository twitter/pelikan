// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Encodes the client side of the memcache service, where we send requests and
//! parse responses.

use service_common::*;
use rustcommon_metrics::*;

use protocol_common::*;
use protocol_ping::*;
use session::Session;

client_counter!(COMPOSE_PING, "compose/ping");
client_counter!(PARSE_PONG, "parse/pong");
client_counter!(PARSE_INVALID, "parse/invalid");

pub struct PingClient {
    parser: ResponseParser,
}

impl From<ResponseParser> for PingClient {
    fn from(other: ResponseParser) -> Self {
        Self {
            parser: other,
        }
    }
}

impl Client<Request, Response> for PingClient {
    fn send(&self, dst: &mut Session, req: &Request) {
        match req {
            Request::Ping => {
                COMPOSE_PING.increment();
            }
        }
        req.compose(dst)
    }

    fn recv(&self, src: &[u8], _req: &Request) -> Result<ParseOk<Response>, ParseError> {
        let res = self.parser.parse(src)?;

        let consumed = res.consumed();
        let res = res.into_inner();

        match res {
            Response::Pong => {
                PARSE_PONG.increment();
            }
        }

        Ok(ParseOk::new(res, consumed))
    }
}
