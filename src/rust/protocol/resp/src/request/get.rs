// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use super::*;

#[derive(Debug, PartialEq, Eq)]
pub struct Get {
    key: Box<[u8]>,
}

impl Get {
    pub fn key(&self) -> &[u8] {
        &self.key
    }
}

impl RequestParser {
    // this is to be called after parsing the command, so we do not match the verb
    pub(crate) fn parse_get_no_stats<'a>(&self, input: &'a [u8]) -> IResult<&'a [u8], Get> {
        let (input, _) = space1(input)?;
        let (input, key) = string(input, self.max_key_len)?;
        let (input, _) = space0(input)?;
        let (input, _) = crlf(input)?;
        Ok((
            input,
            Get {
                key: key.to_owned().into_boxed_slice(),
            },
        ))
    }

    // this is to be called after parsing the command, so we do not match the verb
    pub fn parse_get<'a>(&self, input: &'a [u8]) -> IResult<&'a [u8], Get> {
        match self.parse_get_no_stats(input) {
            Ok((input, request)) => {
                PARSE_GET.increment();
                Ok((input, request))
            }
            Err(e) => {
                if !e.is_incomplete() {
                    PARSE_GET.increment();
                    PARSE_GET_EX.increment();
                }
                Err(e)
            }
        }
    }
}

impl Compose for Get {
    fn compose(&self, session: &mut session::Session) {
        COMPOSE_GET.increment();
        let _ = session.write_all(b"GET \"");
        let _ = session.write_all(&self.key);
        let _ = session.write_all(b"\"\r\n");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse() {
        let parser = RequestParser::new();

        assert_eq!(
            command(b"get key\r\n"),
            Ok((&b" key\r\n"[..], Command::Get))
        );
        assert_eq!(
            parser.parse_get(b" key\r\n"),
            Ok((
                &b""[..],
                Get {
                    key: b"key".to_vec().into_boxed_slice(),
                }
            ))
        );

        // test parsing the entire request in one pass
        assert_eq!(
            parser.parse_request(b"GET key\r\n"),
            Ok((
                &b""[..],
                Request::Get(Get {
                    key: b"key".to_vec().into_boxed_slice(),
                })
            ))
        );

        // test parsing with a binary key
        assert_eq!(
            parser.parse_request(b"GET \"\0\r\n key\" \r\n"),
            Ok((
                &b""[..],
                Request::Get(Get {
                    key: b"\0\r\n key".to_vec().into_boxed_slice(),
                })
            ))
        );
    }
}
