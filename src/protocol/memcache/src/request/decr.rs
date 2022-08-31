// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use super::*;

#[derive(Debug, PartialEq, Eq)]
pub struct Decr {
    pub(crate) key: Box<[u8]>,
    pub(crate) value: u64,
    pub(crate) noreply: bool,
}

impl Decr {
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
    pub fn parse_decr<'a>(&self, input: &'a [u8]) -> IResult<&'a [u8], Decr> {
        // we can use the incr parser here and convert the request
        match self.parse_incr_no_stats(input) {
            Ok((input, request)) => {
                DECR.increment();
                Ok((
                    input,
                    Decr {
                        key: request.key,
                        value: request.value,
                        noreply: request.noreply,
                    },
                ))
            }
            Err(e) => {
                if !e.is_incomplete() {
                    DECR.increment();
                    DECR_EX.increment();
                }
                Err(e)
            }
        }
    }
}

impl Compose for Decr {
    fn compose(&self, session: &mut dyn BufMut) -> usize {
        let verb = b"decr ";
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

impl Klog for Decr {
    type Response = Response;

    fn klog(&self, response: &Self::Response) {
        let (code, len) = match response {
            Response::Numeric(ref res) => {
                DECR_STORED.increment();
                (STORED, res.len())
            }
            Response::NotFound(ref res) => {
                DECR_NOT_FOUND.increment();
                (NOT_FOUND, res.len())
            }
            _ => {
                return;
            }
        };
        klog!("\"decr {}\" {} {}", string_key(self.key()), code, len);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse() {
        let parser = RequestParser::new();

        // basic decr command
        assert_eq!(
            parser.parse_request(b"decr 0 1\r\n"),
            Ok((
                &b""[..],
                Request::Decr(Decr {
                    key: b"0".to_vec().into_boxed_slice(),
                    value: 1,
                    noreply: false,
                })
            ))
        );
    }
}
