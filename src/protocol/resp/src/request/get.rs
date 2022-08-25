// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use std::sync::Arc;
use super::*;

#[derive(Debug, PartialEq, Eq)]
#[allow(clippy::redundant_allocation)]
pub struct GetRequest {
    key: Arc<Box<[u8]>>,
}

impl TryFrom<Message> for GetRequest {
    type Error = ParseError;

    fn try_from(other: Message) -> Result<Self, ParseError> {
        if let Message::Array(array) = other {
            if array.inner.is_none() {
                return Err(ParseError::Invalid);
            }

            let mut array = array.inner.unwrap();

            if array.len() != 2 {
                return Err(ParseError::Invalid);
            }

            let key = if let Message::BulkString(key) = array.remove(1) {
                if key.inner.is_none() {
                    return Err(ParseError::Invalid);
                }

                let key = key.inner.unwrap();

                if key.len() == 0 {
                    return Err(ParseError::Invalid);
                }

                key
            } else {
                return Err(ParseError::Invalid);
            };

            Ok(Self { key })
        } else {
            Err(ParseError::Invalid)
        }
    }
}

impl GetRequest {
    pub fn new(key: &[u8]) -> Self {
        Self {
            key: Arc::new(key.to_owned().into_boxed_slice()),
        }
    }

    pub fn key(&self) -> &[u8] {
        &self.key
    }
}

impl From<&GetRequest> for Message {
    fn from(other: &GetRequest) -> Message {
        Message::Array(Array {
            inner: Some(vec![
                Message::BulkString(BulkString::new(b"GET")),
                Message::BulkString(BulkString::from(other.key.clone())),
            ]),
        })
    }
}

impl Compose for GetRequest {
    fn compose(&self, session: &mut session::Session) {
        let message = Message::from(self);
        message.compose(session)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser() {
        let parser = RequestParser::new();
        assert_eq!(
            parser.parse(b"get 0\r\n").unwrap().into_inner(),
            Request::Get(GetRequest::new(b"0"))
        );

        assert_eq!(
            parser
                .parse(b"get \"\0\r\n key\"\r\n")
                .unwrap()
                .into_inner(),
            Request::Get(GetRequest::new(b"\0\r\n key"))
        );

        assert_eq!(
            parser
                .parse(b"*2\r\n$3\r\nget\r\n$1\r\n0\r\n")
                .unwrap()
                .into_inner(),
            Request::Get(GetRequest::new(b"0"))
        );
    }
}
