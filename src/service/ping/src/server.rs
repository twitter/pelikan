// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use rustcommon_metrics::*;
use service_common::*;

use protocol_common::*;
use protocol_ping::*;
use session::Session;

server_counter!(PARSE_PING, "parse/ping");
server_counter!(COMPOSE_PONG, "compose/pong");

#[derive(Clone)]
pub struct PingServer {
    parser: RequestParser,
}

impl PingServer {
    pub fn new() -> Self {
        Self {
            parser: RequestParser::new(),
        }
    }
}

impl From<RequestParser> for PingServer {
    fn from(other: RequestParser) -> Self {
        Self { parser: other }
    }
}

impl Default for PingServer {
    fn default() -> Self {
        Self::new()
    }
}

impl Server<Request, Response> for PingServer {
    fn recv(&self, src: &[u8]) -> Result<ParseOk<Request>, ParseError> {
        let req = self.parser.parse(src)?;

        let consumed = req.consumed();
        let req = req.into_inner();

        match req {
            Request::Ping => {
                PARSE_PING.increment();
            }
        }

        Ok(ParseOk::new(req, consumed))
    }

    fn send(&self, dst: &mut Session, _req: Request, res: Response) {
        match res {
            Response::Pong => {
                COMPOSE_PONG.increment();
            }
        }
        res.compose(dst)
    }
}
