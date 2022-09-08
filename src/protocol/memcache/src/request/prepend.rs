// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use super::*;

#[derive(Debug, PartialEq, Eq)]
pub struct Prepend {
    pub(crate) key: Box<[u8]>,
    pub(crate) value: Box<[u8]>,
    pub(crate) flags: u32,
    pub(crate) ttl: Option<u32>,
    pub(crate) noreply: bool,
}

impl Prepend {
    pub fn key(&self) -> &[u8] {
        &self.key
    }

    pub fn value(&self) -> &[u8] {
        &self.value
    }

    pub fn ttl(&self) -> Option<u32> {
        self.ttl
    }

    pub fn flags(&self) -> u32 {
        self.flags
    }

    pub fn noreply(&self) -> bool {
        self.noreply
    }
}

impl RequestParser {
    // this is to be called after parsing the command, so we do not match the verb
    pub fn parse_prepend<'a>(&self, input: &'a [u8]) -> IResult<&'a [u8], Prepend> {
        // we can use the set parser here and convert the request
        match self.parse_set_no_stats(input) {
            Ok((input, request)) => {
                PREPEND.increment();
                Ok((
                    input,
                    Prepend {
                        key: request.key,
                        value: request.value,
                        ttl: request.ttl,
                        flags: request.flags,
                        noreply: request.noreply,
                    },
                ))
            }
            Err(e) => {
                if !e.is_incomplete() {
                    PREPEND.increment();
                    PREPEND_EX.increment();
                }
                Err(e)
            }
        }
    }
}

impl Compose for Prepend {
    fn compose(&self, session: &mut dyn BufMut) -> usize {
        let verb = b"prepend ";
        let flags = format!(" {}", self.flags).into_bytes();
        let ttl = convert_ttl(self.ttl);
        let vlen = format!(" {}", self.value.len()).into_bytes();
        let header_end = if self.noreply {
            " noreply\r\n".as_bytes()
        } else {
            "\r\n".as_bytes()
        };

        let size = verb.len()
            + self.key.len()
            + flags.len()
            + ttl.len()
            + vlen.len()
            + header_end.len()
            + self.value.len()
            + CRLF.len();

        session.put_slice(verb);
        session.put_slice(&self.key);
        session.put_slice(&flags);
        session.put_slice(&ttl);
        session.put_slice(&vlen);
        session.put_slice(header_end);
        session.put_slice(&self.value);
        session.put_slice(CRLF);

        size
    }
}

impl Klog for Prepend {
    type Response = Response;

    fn klog(&self, response: &Self::Response) {
        let ttl: i64 = match self.ttl() {
            None => 0,
            Some(0) => -1,
            Some(t) => t as _,
        };
        let (code, len) = match response {
            Response::Stored(ref res) => {
                PREPEND_STORED.increment();
                (STORED, res.len())
            }
            Response::NotStored(ref res) => {
                PREPEND_NOT_STORED.increment();
                (NOT_STORED, res.len())
            }
            _ => {
                return;
            }
        };
        klog!(
            "\"prepend {} {} {} {}\" {} {}",
            string_key(self.key()),
            self.flags(),
            ttl,
            self.value().len(),
            code,
            len
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse() {
        let parser = RequestParser::new();

        // basic prepend command
        assert_eq!(
            parser.parse_request(b"prepend 0 0 0 1\r\n0\r\n"),
            Ok((
                &b""[..],
                Request::Prepend(Prepend {
                    key: b"0".to_vec().into_boxed_slice(),
                    value: b"0".to_vec().into_boxed_slice(),
                    flags: 0,
                    ttl: None,
                    noreply: false,
                })
            ))
        );

        // noreply
        assert_eq!(
            parser.parse_request(b"prepend 0 0 0 1 noreply\r\n0\r\n"),
            Ok((
                &b""[..],
                Request::Prepend(Prepend {
                    key: b"0".to_vec().into_boxed_slice(),
                    value: b"0".to_vec().into_boxed_slice(),
                    flags: 0,
                    ttl: None,
                    noreply: true,
                })
            ))
        );
    }
}
