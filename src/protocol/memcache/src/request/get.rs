// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use super::*;

#[derive(Debug, PartialEq, Eq)]
pub struct Get {
    pub(crate) keys: Box<[Box<[u8]>]>,
}

impl Keys for Get {
    fn keys(&self) -> &[Box<[u8]>] {
        self.keys.as_ref()
    }
}

impl Display for Get {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "get")
    }
}

impl RequestParser {
    // this is to be called after parsing the command, so we do not match the verb
    pub(crate) fn parse_get<'a>(&self, input: &'a [u8]) -> IResult<&'a [u8], Get> {
        let mut keys = Vec::new();

        let (mut input, _) = space1(input)?;

        loop {
            let (i, key) = key(input, self.max_key_len)?;

            match key {
                Some(k) => {
                    keys.push(k.to_owned().into_boxed_slice());
                }
                None => {
                    break;
                }
            };

            if let Ok((i, _)) = space1(i) {
                input = i;
            } else {
                input = i;
                break;
            }

            if keys.len() >= self.max_batch_size {
                return Err(nom::Err::Failure((input, nom::error::ErrorKind::Tag)));
            }
        }

        if keys.is_empty() {
            return Err(nom::Err::Failure((input, nom::error::ErrorKind::Tag)));
        }

        let (input, _) = space0(input)?;
        let (input, _) = crlf(input)?;
        Ok((
            input,
            Get {
                keys: keys.to_owned().into_boxed_slice(),
            },
        ))
    }
}

impl Compose for Get {
    fn compose(&self, session: &mut session::Session) {
        let _ = session.write_all(b"get");
        for key in self.keys.iter() {
            let _ = session.write_all(b" ");
            let _ = session.write_all(key);
        }
        let _ = session.write_all(b"\r\n");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse() {
        let parser = RequestParser::new();

        // basic get command
        assert_eq!(
            parser.parse_request(b"get key\r\n"),
            Ok((
                &b""[..],
                Request::Get(Get {
                    keys: vec![b"key".to_vec().into_boxed_slice()].into_boxed_slice(),
                })
            ))
        );

        // command name is not case sensitive
        assert_eq!(
            parser.parse_request(b"get key \r\n"),
            parser.parse_request(b"GET key \r\n"),
        );

        // trailing spaces don't matter
        assert_eq!(
            parser.parse_request(b"get key\r\n"),
            parser.parse_request(b"get key \r\n"),
        );

        // multiple trailing spaces is fine too
        assert_eq!(
            parser.parse_request(b"get key\r\n"),
            parser.parse_request(b"get key      \r\n"),
        );

        // request can have multiple keys
        assert_eq!(
            parser.parse_request(b"get a b c\r\n"),
            Ok((
                &b""[..],
                Request::Get(Get {
                    keys: vec![
                        b"a".to_vec().into_boxed_slice(),
                        b"b".to_vec().into_boxed_slice(),
                        b"c".to_vec().into_boxed_slice(),
                    ]
                    .into_boxed_slice(),
                })
            ))
        );

        // key is binary safe
        assert_eq!(
            parser.parse_request(b"get evil\0key \r\n"),
            Ok((
                &b""[..],
                Request::Get(Get {
                    keys: vec![b"evil\0key".to_vec().into_boxed_slice(),].into_boxed_slice()
                })
            ))
        );
    }
}