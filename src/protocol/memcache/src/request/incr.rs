// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use super::*;

#[derive(Debug, PartialEq, Eq)]
pub struct Incr {
    pub(crate) key: Box<[u8]>,
    pub(crate) value: u64,
    pub(crate) noreply: bool,
}

impl Incr {
    pub fn key(&self) -> &[u8] {
        self.key.as_ref()
    }

    pub fn value(&self) -> u64 {
        self.value
    }

    pub fn noreply(&self) -> bool {
        self.noreply
    }
}

impl RequestParser {
    // this is to be called after parsing the command, so we do not match the verb
    pub(crate) fn parse_incr<'a>(&self, input: &'a [u8]) -> IResult<&'a [u8], Incr> {
        let mut noreply = false;

        let (input, _) = space1(input)?;
        let (input, key) = key(input, self.max_key_len)?;

        let key = match key {
            Some(k) => k,
            None => {
                return Err(nom::Err::Failure((input, nom::error::ErrorKind::Tag)));
            }
        };

        let (input, _) = space1(input)?;
        let (mut input, value) = parse_u64(input)?;

        // if we have a space, we might have a noreply
        if let Ok((i, _)) = space1(input) {
            if i.len() > 7 && &i[0..7] == b"noreply" {
                input = &i[7..];
                noreply = true;
            }
        }

        let (input, _) = space0(input)?;
        let (input, _) = crlf(input)?;

        Ok((
            input,
            Incr {
                key: key.to_owned().into_boxed_slice(),
                value,
                noreply,
            },
        ))
    }
}

impl Compose for Incr {
    fn compose(&self, session: &mut session::Session) {
        let _ = session.write_all(b"incr ");
        let _ = session.write_all(&self.key);
        let _ = session.write_all(format!(" {}", self.value).as_bytes());
        if self.noreply {
            let _ = session.write_all(b" noreply\r\n");
        } else {
            let _ = session.write_all(b"\r\n");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse() {
        let parser = RequestParser::new();

        // basic command
        assert_eq!(
            parser.parse_request(b"incr 0 1\r\n"),
            Ok((
                &b""[..],
                Request::Incr(Incr {
                    key: b"0".to_vec().into_boxed_slice(),
                    value: 1,
                    noreply: false,
                })
            ))
        );

        // noreply
        assert_eq!(
            parser.parse_request(b"incr 0 1 noreply\r\n"),
            Ok((
                &b""[..],
                Request::Incr(Incr {
                    key: b"0".to_vec().into_boxed_slice(),
                    value: 1,
                    noreply: true,
                })
            ))
        );

        // alternate value
        assert_eq!(
            parser.parse_request(b"incr 0 42\r\n"),
            Ok((
                &b""[..],
                Request::Incr(Incr {
                    key: b"0".to_vec().into_boxed_slice(),
                    value: 42,
                    noreply: false,
                })
            ))
        );

        // trailing space doesn't matter
        assert_eq!(
            parser.parse_request(b"incr 0 1\r\n"),
            parser.parse_request(b"incr 0 1 \r\n"),
        );
        assert_eq!(
            parser.parse_request(b"incr 0 1 noreply\r\n"),
            parser.parse_request(b"incr 0 1 noreply \r\n"),
        );
    }
}
