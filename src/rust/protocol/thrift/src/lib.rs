use logger::*;
use protocol_common::Parse;
use protocol_common::{ParseOk, ParseError};
use std::io::Write;
use protocol_common::Compose;

/// An opaque Thrift message
pub struct Message {
    data: Box<[u8]>,
}

impl Compose for Message {
    fn compose(&self, session: &mut session::Session) {
        debug!("message size: {} bytes framed size: {}", self.data.len(), self.data.len() + 4);
        let initial = session.write_pending();
        debug!("session had: {} bytes in the write buffer", initial);
        let _ = session.write_all(&(self.data.len() as u32).to_be_bytes());
        let _ = session.write_all(&self.data);
        let written = session.write_pending() - initial;

        debug!("wrote: {} bytes to session", written);
    }
}

/// A parser which retrieves the bytes for a complete Thrift message.
#[derive(Clone)]
pub struct MessageParser {
    max_size: usize,
}

impl MessageParser {
    pub fn new(max_size: usize) -> Self {
        Self {
            max_size
        }
    }
}

impl Parse<Message> for MessageParser {
    fn parse(&self, buffer: &[u8]) -> Result<ParseOk<Message>, ParseError> {
        if buffer.len() < 4 {
            return Err(ParseError::Incomplete);
        }

        let data_len = u32::from_be_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]);

        let framed_len = 4 + data_len as usize;

        if framed_len > self.max_size {
            return Err(ParseError::Invalid);
        }

        if buffer.len() < framed_len {
            trace!("incomplete response have: {} bytes need: {}", buffer.len(), framed_len);
            Err(ParseError::Incomplete)
        } else {
            debug!("message size: {} bytes framed size: {}", data_len, framed_len);
            let data = buffer[4..framed_len].to_vec().into_boxed_slice();
            let message = Message { data };
            Ok(ParseOk::new(message, framed_len))
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
