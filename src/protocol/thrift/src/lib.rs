// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! A protocol crate for Thrift binary protocol.

use protocol_common::BufMut;
use protocol_common::Compose;
use protocol_common::Parse;
use protocol_common::ParseOk;
use rustcommon_metrics::*;

const THRIFT_HEADER_LEN: usize = std::mem::size_of::<u32>();

// Stats
counter!(MESSAGES_PARSED);
counter!(MESSAGES_COMPOSED);

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
    fn compose(&self, session: &mut dyn BufMut) -> usize {
        MESSAGES_COMPOSED.increment();
        session.put_slice(&(self.data.len() as u32).to_be_bytes());
        session.put_slice(&self.data);
        std::mem::size_of::<u32>() + self.data.len()
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
    fn parse(&self, buffer: &[u8]) -> Result<ParseOk<Message>, std::io::Error> {
        if buffer.len() < THRIFT_HEADER_LEN {
            return Err(std::io::Error::from(std::io::ErrorKind::WouldBlock));
        }

        let data_len = u32::from_be_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]);

        let framed_len = THRIFT_HEADER_LEN + data_len as usize;

        if framed_len == 0 || framed_len > self.max_size {
            return Err(std::io::Error::from(std::io::ErrorKind::InvalidInput));
        }

        if buffer.len() < framed_len {
            Err(std::io::Error::from(std::io::ErrorKind::WouldBlock))
        } else {
            MESSAGES_PARSED.increment();
            let data = buffer[THRIFT_HEADER_LEN..framed_len]
                .to_vec()
                .into_boxed_slice();
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

        assert_eq!(consumed, body.len() + THRIFT_HEADER_LEN);
        assert_eq!(*parsed.data, body);
    }
}

common::metrics::test_no_duplicates!();
