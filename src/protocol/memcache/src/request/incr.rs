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
    pub(crate) fn parse_incr_no_stats<'a>(&self, input: &'a [u8]) -> IResult<&'a [u8], Incr> {
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

    pub fn parse_incr<'a>(&self, input: &'a [u8]) -> IResult<&'a [u8], Incr> {
        match self.parse_incr_no_stats(input) {
            Ok((input, request)) => {
                INCR.increment();
                Ok((input, request))
            }
            Err(e) => {
                if !e.is_incomplete() {
                    INCR.increment();
                    INCR_EX.increment();
                }
                Err(e)
            }
        }
    }
}

impl Compose for Incr {
    fn compose(&self, session: &mut dyn BufMut) -> usize {
        let verb = b"incr ";
        let value = format!(" {}", self.value).into_bytes();
        let header_end = if self.noreply {
            " noreply\r\n".as_bytes()
        } else {
            "\r\n".as_bytes()
        };

        let size = verb.len() + self.key.len() + value.len() + header_end.len();

        session.put_slice(verb);
        session.put_slice(&self.key);
        session.put_slice(&value);
        session.put_slice(header_end);

        size
    }
}

impl Klog for Incr {
    type Response = Response;

    fn klog(&self, response: &Self::Response) {
        let (code, len) = match response {
            Response::Numeric(ref res) => {
                INCR_STORED.increment();
                (STORED, res.len())
            }
            Response::NotFound(ref res) => {
                INCR_NOT_FOUND.increment();
                (NOT_STORED, res.len())
            }
            _ => {
                return;
            }
        };
        klog!("\"incr {}\" {} {}", string_key(self.key()), code, len);
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
