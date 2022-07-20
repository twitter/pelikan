// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use super::*;

#[derive(Debug, PartialEq, Eq)]
pub struct Delete {
    pub(crate) key: Box<[u8]>,
    pub(crate) noreply: bool,
}

impl Delete {
    pub fn key(&self) -> &[u8] {
        self.key.as_ref()
    }

    pub fn noreply(&self) -> bool {
        self.noreply
    }
}

impl RequestParser {
    // this is to be called after parsing the command, so we do not match the verb
    pub(crate) fn parse_delete_no_stats<'a>(&self, input: &'a [u8]) -> IResult<&'a [u8], Delete> {
        let (input, _) = space1(input)?;

        let (mut input, key) = key(input, self.max_key_len)?;

        let key = match key {
            Some(k) => k,
            None => {
                return Err(nom::Err::Failure((input, nom::error::ErrorKind::Tag)));
            }
        };

        let mut noreply = false;

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
            Delete {
                key: key.to_owned().into_boxed_slice(),
                noreply,
            },
        ))
    }

    // this is to be called after parsing the command, so we do not match the verb
    pub fn parse_delete<'a>(&self, input: &'a [u8]) -> IResult<&'a [u8], Delete> {
        match self.parse_delete_no_stats(input) {
            Ok((input, request)) => {
                DELETE.increment();
                Ok((input, request))
            }
            Err(e) => {
                if !e.is_incomplete() {
                    DELETE.increment();
                    DELETE_EX.increment();
                }
                Err(e)
            }
        }
    }
}

impl Compose for Delete {
    fn compose(&self, session: &mut dyn BufMut) -> usize {
        let verb = b"delete ";
        let header_end = if self.noreply {
            " noreply\r\n".as_bytes()
        } else {
            "\r\n".as_bytes()
        };

        let size = verb.len() + self.key.len() + header_end.len();

        session.put_slice(verb);
        session.put_slice(&self.key);
        session.put_slice(header_end);

        size
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse() {
        let parser = RequestParser::new();

        // basic delete command
        assert_eq!(
            parser.parse_request(b"delete 0\r\n"),
            Ok((
                &b""[..],
                Request::Delete(Delete {
                    key: b"0".to_vec().into_boxed_slice(),
                    noreply: false,
                })
            ))
        );
    }
}
