// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use super::*;

#[derive(Debug, PartialEq, Eq)]
pub struct Cas {
    pub(crate) key: Box<[u8]>,
    pub(crate) value: Box<[u8]>,
    pub(crate) flags: u32,
    pub(crate) ttl: Ttl,
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

    pub fn ttl(&self) -> Ttl {
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
    pub(crate) fn parse_cas_no_stats<'a>(&self, input: &'a [u8]) -> IResult<&'a [u8], Cas> {
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
        let (input, ttl) = parse_ttl(input, self.time_type)?;
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

    // this is to be called after parsing the command, so we do not match the verb
    pub fn parse_cas<'a>(&self, input: &'a [u8]) -> IResult<&'a [u8], Cas> {
        match self.parse_cas_no_stats(input) {
            Ok((input, request)) => {
                CAS.increment();
                Ok((input, request))
            }
            Err(e) => {
                if !e.is_incomplete() {
                    CAS.increment();
                    CAS_EX.increment();
                }
                Err(e)
            }
        }
    }
}

impl Compose for Cas {
    fn compose(&self, session: &mut dyn BufMut) -> usize {
        let verb = b"cas ";
        let flags = format!(" {}", self.flags).into_bytes();
        let ttl = format!(" {}", self.ttl.get().unwrap_or(0)).into_bytes();
        let vlen = format!(" {}", self.value.len()).into_bytes();
        let cas = format!(" {}", self.cas).into_bytes();
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
            + cas.len()
            + header_end.len()
            + self.value.len()
            + CRLF.len();

        session.put_slice(verb);
        session.put_slice(&self.key);
        session.put_slice(&flags);
        session.put_slice(&ttl);
        session.put_slice(&vlen);
        session.put_slice(&cas);
        session.put_slice(header_end);
        session.put_slice(&self.value);
        session.put_slice(CRLF);

        size
    }
}

impl Klog for Cas {
    type Response = Response;

    fn klog(&self, response: &Self::Response) {
        let (code, len) = match response {
            Response::Stored(ref res) => {
                CAS_STORED.increment();
                (STORED, res.len())
            }
            Response::Exists(ref res) => {
                CAS_EXISTS.increment();
                (EXISTS, res.len())
            }
            Response::NotFound(ref res) => {
                CAS_NOT_FOUND.increment();
                (NOT_FOUND, res.len())
            }
            _ => {
                return;
            }
        };
        klog!(
            "\"cas {} {} {} {} {}\" {} {}",
            string_key(self.key()),
            self.flags(),
            self.ttl.get().unwrap_or(0),
            self.value().len(),
            self.cas(),
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

        // basic cas command
        assert_eq!(
            parser.parse_request(b"cas 0 0 0 1 42\r\n0\r\n"),
            Ok((
                &b""[..],
                Request::Cas(Cas {
                    key: b"0".to_vec().into_boxed_slice(),
                    value: b"0".to_vec().into_boxed_slice(),
                    flags: 0,
                    ttl: Ttl::none(),
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
                    ttl: Ttl::none(),
                    cas: 42,
                    noreply: true,
                })
            ))
        );
    }
}
