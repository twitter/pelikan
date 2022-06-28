// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! A protocol crate for Thrift binary protocol.

use protocol_common::Compose;
use protocol_common::Parse;
use protocol_common::{ParseError, ParseOk};
use std::io::Write;

/// An opaque Thrift message
pub struct Message {
    data: Box<[u8]>,
}

#[allow(clippy::len_without_is_empty)]
impl Message {
    pub fn len(&self) -> usize {
        self.data.len()
    }
}

impl Compose for Message {
    fn compose(&self, session: &mut session::Session) {
        let _ = session.write_all(&(self.data.len() as u32).to_be_bytes());
        let _ = session.write_all(&self.data);
    }
}

/// A parser which retrieves the bytes for a complete Thrift message.
#[derive(Clone)]
pub struct MessageParser {
    max_size: usize,
}

impl MessageParser {
    pub const fn new(max_size: usize) -> Self {
        Self { max_size }
    }
}

impl Parse<Message> for MessageParser {
    fn parse(&self, buffer: &[u8]) -> Result<ParseOk<Message>, ParseError> {
        if buffer.len() < 4 {
            return Err(ParseError::Incomplete);
        }

        let data_len = u32::from_be_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]);

        let framed_len = 4 + data_len as usize;

        if framed_len == 0 || framed_len > self.max_size {
            return Err(ParseError::Invalid);
        }

        if buffer.len() < framed_len {
            Err(ParseError::Incomplete)
        } else {
            let data = buffer[4..framed_len].to_vec().into_boxed_slice();
            let message = Message { data };
            Ok(ParseOk::new(message, framed_len))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse() {
        let body = b"COFFEE".to_vec();
        let len = (body.len() as u32).to_be_bytes();

        let mut message: Vec<u8> = len.to_vec();
        message.extend_from_slice(&body);

        let parser = MessageParser::new(1024);

        let parsed = parser.parse(&message).expect("failed to parse");
        let consumed = parsed.consumed();
        let parsed = parsed.into_inner();

        assert_eq!(consumed, body.len() + 4);
        assert_eq!(*parsed.data, body);
    }
}
