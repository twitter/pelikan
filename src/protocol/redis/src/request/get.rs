// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use super::*;

#[derive(Debug, PartialEq, Eq)]
pub struct GetRequest {
    key: Box<[u8]>,
}

impl GetRequest {
    pub fn new(key: &[u8]) -> Self {
        Self {
            key: key.to_owned().into_boxed_slice()
        }
    }

    pub(crate) fn from_array(array: &[&[u8]]) -> Result<Self, ()> {
        if array.len() != 2 {
            Err(())
        } else {
            Ok(GetRequest::new(array[1]))
        }
    }

    pub fn key(&self) -> &[u8] {
        &self.key
    }
}

// this is to be called after parsing the command, so we do not match the verb
pub fn parse(input: &[u8]) -> IResult<&[u8], GetRequest> {
    let (input, _) = space1(input)?;
    let (input, key) = string(input)?;
    let (input, _) = space0(input)?;
    let (input, _) = crlf(input)?;
    Ok((
        input,
        GetRequest {
            key: key.to_owned().into_boxed_slice(),
        },
    ))
}

impl Compose for GetRequest {
    fn compose(&self, session: &mut session::Session) {
        let _ = session.write_all(&format!("*2\r\n$3\r\nGET\r\n${}\r\n", self.key.len()).as_bytes());
        let _ = session.write_all(&self.key);
        let _ = session.write_all(b"\r\n");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_inline() {
        assert_eq!(
            inline_command(b"get key\r\n"),
            Ok((&b" key\r\n"[..], Command::Get))
        );
        assert_eq!(
            get::parse(b" key\r\n"),
            Ok((
                &b""[..],
                GetRequest {
                    key: b"key".to_vec().into_boxed_slice(),
                }
            ))
        );

        // test parsing the entire request in one pass
        assert_eq!(
            inline_request(b"GET key\r\n"),
            Ok((
                &b""[..],
                Request::Get(GetRequest {
                    key: b"key".to_vec().into_boxed_slice(),
                })
            ))
        );

        // test parsing with a binary key
        assert_eq!(
            inline_request(b"GET \"\0\r\n key\" \r\n"),
            Ok((
                &b""[..],
                Request::Get(GetRequest {
                    key: b"\0\r\n key".to_vec().into_boxed_slice(),
                })
            ))
        );
    }

    #[test]
    fn parser() {
        let parser = RequestParser {};
        assert_eq!(
            parser.parse(b"get 0\r\n").unwrap().into_inner(),
            Request::Get(GetRequest::new(b"0"))
        );

        assert_eq!(
            parser.parse(b"*2\r\n$3\r\nget\r\n$1\r\n0\r\n").unwrap().into_inner(),
            Request::Get(GetRequest::new(b"0"))
        );
    }
}
