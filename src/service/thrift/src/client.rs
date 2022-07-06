// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Encodes the client side of the memcache service, where we send requests and
//! parse responses.

use rustcommon_metrics::*;
use service_common::*;

use protocol_common::*;
use protocol_thrift::*;
use session::Session;

client_counter!(COMPOSE_MESSAGE, "compose/message");
client_counter!(PARSE_MESSAGE, "parse/message");
client_counter!(PARSE_INVALID, "parse/invalid");

#[derive(Clone)]
pub struct ThriftClient {
    parser: MessageParser,
}

impl ThriftClient {
    pub fn new(max_size: usize) -> Self {
        Self {
            parser: MessageParser::new(max_size),
        }
    }
}

impl From<MessageParser> for ThriftClient {
    fn from(other: MessageParser) -> Self {
        Self { parser: other }
    }
}

impl Client<Message, Message> for ThriftClient {
    fn send(&self, dst: &mut Session, req: &Message) {
        COMPOSE_MESSAGE.increment();
        req.compose(dst)
    }

    fn recv(&self, src: &[u8], _req: &Message) -> Result<ParseOk<Message>, ParseError> {
        let message = self.parser.parse(src)?;

        let consumed = message.consumed();
        let message = message.into_inner();

        PARSE_MESSAGE.increment();

        Ok(ParseOk::new(message, consumed))
    }
}
