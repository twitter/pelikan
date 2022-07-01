// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use super::*;

use common::time::Seconds;
use common::time::UnixInstant;

#[derive(Debug, PartialEq, Eq)]
pub struct Set {
    pub(crate) key: Box<[u8]>,
    pub(crate) value: Box<[u8]>,
    pub(crate) flags: u32,
    pub(crate) ttl: Option<u32>,
    pub(crate) noreply: bool,
}

impl Ttl for Set {
    fn ttl(&self) -> Option<u32> {
        self.ttl
    }
}

impl Key for Set {
    fn key(&self) -> &[u8] {
        &self.key
    }
}

impl NoReply for Set {
    fn noreply(&self) -> bool {
        self.noreply
    }
}

impl RequestValue for Set {
    fn value(&self) -> &[u8] {
        &self.value
    }
}

impl Flags for Set {
    fn flags(&self) -> u32 {
        self.flags
    }
}

impl Display for Set {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "set")
    }
}

impl RequestParser {
    // this is to be called after parsing the command, so we do not match the verb
    pub(crate) fn parse_set<'a>(&self, input: &'a [u8]) -> IResult<&'a [u8], Set> {
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
        let (mut input, bytes) = parse_usize(input)?;

        if bytes > self.max_value_size {
            return Err(nom::Err::Failure((input, nom::error::ErrorKind::Tag)));
        }

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
        } else if self.time_type == TimeType::Unix
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
        };

        Ok((
            input,
            Set {
                key: key.to_owned().into_boxed_slice(),
                value: value.to_owned().into_boxed_slice(),
                ttl,
                flags,
                noreply,
            },
        ))
    }
}

impl Compose for Set {
    fn compose(&self, session: &mut session::Session) {
        let _ = session.write_all(b"set ");
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

        // basic set command
        assert_eq!(
            parser.parse_request(b"set 0 0 0 1\r\n0\r\n"),
            Ok((
                &b""[..],
                Request::Set(Set {
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
            parser.parse_request(b"set 0 0 0 1 noreply\r\n0\r\n"),
            Ok((
                &b""[..],
                Request::Set(Set {
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
