// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use super::*;

#[derive(Debug, PartialEq, Eq)]
pub struct Append {
    pub(crate) key: Box<[u8]>,
    pub(crate) value: Box<[u8]>,
    pub(crate) flags: u32,
    pub(crate) ttl: Option<u32>,
    pub(crate) noreply: bool,
}

impl Ttl for Append {
    fn ttl(&self) -> Option<u32> {
        self.ttl
    }
}

impl Key for Append {
    fn key(&self) -> &[u8] {
        &self.key
    }
}

impl NoReply for Append {
    fn noreply(&self) -> bool {
        self.noreply
    }
}

impl RequestValue for Append {
    fn value(&self) -> &[u8] {
        &self.value
    }
}

impl Flags for Append {
    fn flags(&self) -> u32 {
        self.flags
    }
}

impl Display for Append {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "append")
    }
}

impl RequestParser {
    // this is to be called after parsing the command, so we do not match the verb
    pub fn parse_append<'a>(&self, input: &'a [u8]) -> IResult<&'a [u8], Append> {
        // we can use the set parser here and convert the request
        let (input, request) = self.parse_set(input)?;

        Ok((
            input,
            Append {
                key: request.key,
                value: request.value,
                ttl: request.ttl,
                flags: request.flags,
                noreply: request.noreply,
            },
        ))
    }
}

impl Compose for Append {
    fn compose(&self, session: &mut session::Session) {
        let _ = session.write_all(b"append ");
        let _ = session.write_all(&self.key);
        let _ = session.write_all(format!(" {}", self.flags).as_bytes());
        match self.ttl {
            None => {
                let _ = session.write_all(b" 0");
            }
            Some(0) => {
                let _ = session.write_all(b" -1");
            }
            Some(s) => {
                let _ = session.write_all(format!(" {}", s).as_bytes());
            }
        }
        let _ = session.write_all(format!(" {}\r\n", self.value.len()).as_bytes());
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

        // basic append command
        assert_eq!(
            parser.parse_request(b"append 0 0 0 1\r\n0\r\n"),
            Ok((
                &b""[..],
                Request::Append(Append {
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
            parser.parse_request(b"append 0 0 0 1 noreply\r\n0\r\n"),
            Ok((
                &b""[..],
                Request::Append(Append {
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