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
        let (input, request) = self.parse_set(input)?;
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
}

impl Compose for Prepend {
    fn compose(&self, session: &mut session::Session) {
        let _ = session.write_all(b"prepend ");
        let _ = session.write_all(&self.key);
        let _ = session.write_all(&format!(" {}", self.flags).as_bytes());
        match self.ttl {
            None => {
                let _ = session.write_all(b" 0");
            }
            Some(0) => {
                let _ = session.write_all(b" -1");
            }
            Some(s) => {
                let _ = session.write_all(&format!(" {}", s).as_bytes());
            }
        }
        let _ = session.write_all(&format!(" {}\r\n", self.value.len()).as_bytes());
        let _ = session.write_all(&self.value);
        let _ = session.write_all(b"\r\n");
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
