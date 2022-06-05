// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use super::*;

#[derive(Debug, PartialEq, Eq)]
pub struct Gets {
    pub(crate) keys: Box<[Box<[u8]>]>,
}

impl Gets {
    pub fn keys(&self) -> &[Box<[u8]>] {
        self.keys.as_ref()
    }
}

impl RequestParser {
    // this is to be called after parsing the command, so we do not match the verb
    pub(crate) fn parse_gets<'a>(&self, input: &'a [u8]) -> IResult<&'a [u8], Gets> {
        // we can use the get parser here and convert the request
        let (input, request) = self.parse_get(input)?;
        Ok((input, Gets { keys: request.keys }))
    }
}

impl Compose for Gets {
    fn compose(&self, session: &mut session::Session) {
        let _ = session.write_all(b"gets");
        for key in self.keys.iter() {
            let _ = session.write_all(b" ");
            let _ = session.write_all(&key);
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

        // test parsing a simple request
        assert_eq!(
            parser.parse_request(b"gets key \r\n"),
            Ok((
                &b""[..],
                Request::Gets(Gets {
                    keys: vec![b"key".to_vec().into_boxed_slice()].into_boxed_slice(),
                })
            ))
        );

        // command name is not case sensitive
        assert_eq!(
            parser.parse_request(b"gets key \r\n"),
            parser.parse_request(b"GETS key \r\n"),
        );

        // trailing spaces don't matter
        assert_eq!(
            parser.parse_request(b"gets key\r\n"),
            parser.parse_request(b"gets key \r\n"),
        );

        // multiple trailing spaces is fine too
        assert_eq!(
            parser.parse_request(b"gets key\r\n"),
            parser.parse_request(b"gets key      \r\n"),
        );

        // request can have multiple keys
        assert_eq!(
            parser.parse_request(b"gets a b c\r\n"),
            Ok((
                &b""[..],
                Request::Gets(Gets {
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
            parser.parse_request(b"gets evil\0key \r\n"),
            Ok((
                &b""[..],
                Request::Gets(Gets {
                    keys: vec![b"evil\0key".to_vec().into_boxed_slice(),].into_boxed_slice()
                })
            ))
        );
    }
}
