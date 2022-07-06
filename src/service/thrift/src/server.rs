// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use rustcommon_metrics::*;
use service_common::*;

use protocol_common::*;
use protocol_thrift::*;
use session::Session;

server_counter!(PARSE_MESSAGE, "parse/message");
server_counter!(COMPOSE_MESSAGE, "compose/message");

#[derive(Clone)]
pub struct ThriftServer {
    parser: MessageParser,
}

impl From<MessageParser> for ThriftServer {
    fn from(other: MessageParser) -> Self {
        Self { parser: other }
    }
}

impl ThriftServer {
    pub fn new(max_size: usize) -> Self {
        Self {
            parser: MessageParser::new(max_size),
        }
    }
}

impl Server<Message, Message> for ThriftServer {
    fn recv(&self, src: &[u8]) -> Result<ParseOk<Message>, ParseError> {
        let message = self.parser.parse(src)?;

        let consumed = message.consumed();
        let message = message.into_inner();

        PARSE_MESSAGE.increment();

        Ok(ParseOk::new(message, consumed))
    }

    fn send(&self, dst: &mut Session, _req: Message, res: Message) {
        COMPOSE_MESSAGE.increment();
        res.compose(dst)
    }
}
