// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use super::*;
use common::time::Seconds;
use common::time::UnixInstant;

#[derive(Debug, PartialEq, Eq)]
pub struct Cas {
    pub(crate) key: Box<[u8]>,
    pub(crate) value: Box<[u8]>,
    pub(crate) flags: u32,
    pub(crate) ttl: Option<u32>,
    pub(crate) cas: u64,
    pub(crate) noreply: bool,
}

impl Cas {
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

    pub fn cas(&self) -> u64 {
        self.cas
    }

    pub fn noreply(&self) -> bool {
        self.noreply
    }
}

impl RequestParser {
    // this is to be called after parsing the command, so we do not match the verb
    pub fn parse_cas<'a>(&self, input: &'a [u8]) -> IResult<&'a [u8], Cas> {
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
        let (input, flags) = parse_u32(input)?;
        let (input, _) = space1(input)?;
        let (input, exptime) = parse_i64(input)?;
        let (input, _) = space1(input)?;
        let (input, bytes) = parse_usize(input)?;

        if bytes > self.max_value_size {
            return Err(nom::Err::Failure((input, nom::error::ErrorKind::Tag)));
        }

        let (input, _) = space1(input)?;
        let (mut input, cas) = parse_u64(input)?;

        // if we have a space, we might have a noreply
        if let Ok((i, _)) = space1(input) {
            if i.len() > 7 && &i[0..7] == b"noreply" {
                input = &i[7..];
                noreply = true;
            }
        }

        let (input, _) = space0(input)?;
        let (input, _) = crlf(input)?;
        let (input, value) = take(bytes)(input)?;
        let (input, _) = crlf(input)?;

        let ttl = if exptime < 0 {
            Some(0)
        } else if exptime == 0 {
            None
        } else {
            if self.time_type == TimeType::Unix
                || (self.time_type == TimeType::Memcache && exptime > 60 * 60 * 24 * 30)
            {
                Some(
                    UnixInstant::from_secs(exptime as u32)
                        .checked_duration_since(UnixInstant::<Seconds<u32>>::recent())
                        .map(|v| v.as_secs())
                        .unwrap_or(0),
                )
            } else {
                Some(exptime as u32)
            }
        };

        Ok((
            input,
            Cas {
                key: key.to_owned().into_boxed_slice(),
                value: value.to_owned().into_boxed_slice(),
                ttl,
                flags,
                cas,
                noreply,
            },
        ))
    }
}

impl Compose for Cas {
    fn compose(&self, session: &mut session::Session) {
        let _ = session.write_all(b"cas ");
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
        let _ = session.write_all(&format!(" {} {}\r\n", self.value.len(), self.cas).as_bytes());
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

        // basic cas command
        assert_eq!(
            parser.parse_request(b"cas 0 0 0 1 42\r\n0\r\n"),
            Ok((
                &b""[..],
                Request::Cas(Cas {
                    key: b"0".to_vec().into_boxed_slice(),
                    value: b"0".to_vec().into_boxed_slice(),
                    flags: 0,
                    ttl: None,
                    cas: 42,
                    noreply: false,
                })
            ))
        );

        // noreply
        assert_eq!(
            parser.parse_request(b"cas 0 0 0 1 42 noreply\r\n0\r\n"),
            Ok((
                &b""[..],
                Request::Cas(Cas {
                    key: b"0".to_vec().into_boxed_slice(),
                    value: b"0".to_vec().into_boxed_slice(),
                    flags: 0,
                    ttl: None,
                    cas: 42,
                    noreply: true,
                })
            ))
        );
    }
}
