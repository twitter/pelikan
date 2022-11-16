use super::*;
use std::io::{Error, ErrorKind};
use std::sync::Arc;

type ArcByteSlice = Arc<Box<[u8]>>;
#[derive(Debug, PartialEq, Eq)]
pub struct HmGetRequest {
    key: ArcByteSlice,
    fields: Arc<Box<[ArcByteSlice]>>,
}

impl HmGetRequest {
    pub fn key(&self) -> &[u8] {
        &self.key
    }

    pub fn fields(&self) -> Box<[&[u8]]> {
        self.fields
            .iter()
            .map(|f| &***f)
            .collect::<Vec<&[u8]>>()
            .into_boxed_slice()
    }
}

impl TryFrom<Message> for HmGetRequest {
    type Error = Error;

    fn try_from(other: Message) -> Result<Self, Error> {
        if let Message::Array(array) = other {
            if array.inner.is_none() {
                return Err(Error::new(ErrorKind::Other, "malformed command"));
            }

            let mut array = array.inner.unwrap();

            if array.len() <= 2 {
                return Err(Error::new(ErrorKind::Other, "malformed command"));
            }

            let key = take_bulk_string(&mut array)?;
            if key.is_empty() {
                return Err(Error::new(ErrorKind::Other, "malformed command"));
            }

            let mut fields = Vec::with_capacity(array.len());
            while array.len() >= 2 {
                let field = take_bulk_string(&mut array)?;
                if field.is_empty() {
                    return Err(Error::new(ErrorKind::Other, "malformed command"));
                }

                fields.push(field);
            }

            let f = Arc::new(Box::<[ArcByteSlice]>::from(fields));
            Ok(Self { key, fields: f })
        } else {
            Err(Error::new(ErrorKind::Other, "malformed command"))
        }
    }
}

impl From<&HmGetRequest> for Message {
    fn from(other: &HmGetRequest) -> Message {
        let mut v = vec![
            Message::bulk_string(b"HMGET"),
            Message::BulkString(BulkString::from(other.key.clone())),
        ];
        for kv in (*other.fields).iter() {
            v.push(Message::BulkString(BulkString::from(kv.clone())));
        }

        Message::Array(Array { inner: Some(v) })
    }
}

impl Compose for HmGetRequest {
    fn compose(&self, buf: &mut dyn BufMut) -> usize {
        let message = Message::from(self);
        message.compose(buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser() {
        let parser = RequestParser::new();

        //1 field
        if let Request::HmGet(request) = parser.parse(b"hmget key field1\r\n").unwrap().into_inner()
        {
            assert_eq!(request.key(), b"key");
            assert_eq!(request.fields().len(), 1);
            assert_eq!(request.fields()[0], b"field1");
        } else {
            panic!("invalid parse result");
        }

        //2 fields
        if let Request::HmGet(request) = parser
            .parse(b"hmget key field1 field2\r\n")
            .unwrap()
            .into_inner()
        {
            assert_eq!(request.key(), b"key");
            assert_eq!(request.fields().len(), 2);
            assert_eq!(request.fields()[0], b"field1");
            assert_eq!(request.fields()[1], b"field2");
        } else {
            panic!("invalid parse result");
        }

        //3 fields
        if let Request::HmGet(request) = parser
            .parse(b"hmget key field1 field2 42\r\n")
            .unwrap()
            .into_inner()
        {
            assert_eq!(request.key(), b"key");
            assert_eq!(request.fields().len(), 3);
            assert_eq!(request.fields()[0], b"field1");
            assert_eq!(request.fields()[1], b"field2");
            assert_eq!(request.fields()[2], b"42");
        } else {
            panic!("invalid parse result");
        }

        //insufficient whitespace delimited strings
        parser
            .parse(b"hmget key\r\n")
            .expect_err("malformed command");
    }
}
