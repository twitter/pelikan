// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use super::*;
use protocol_common::Compose;

#[derive(Debug, PartialEq, Eq)]
pub struct Array {
    pub(crate) inner: Option<Vec<Message>>,
}

impl Compose for Array {
    fn compose(&self, session: &mut dyn BufMut) -> usize {
        let mut len = 0;
        if let Some(values) = &self.inner {
            let header = format!("${}\r\n", values.len());
            session.put_slice(header.as_bytes());
            len += header.as_bytes().len();
            for value in values {
                len += value.compose(session);
            }
            session.put_slice(b"\r\n");
            len += 2;
        } else {
            session.put_slice(b"*-1\r\n");
            len += 5;
        }
        len
    }
}

pub fn parse(input: &[u8]) -> IResult<&[u8], Array> {
    match input.first() {
        Some(b'-') => {
            let (input, _) = take(1usize)(input)?;
            let (input, len) = digit1(input)?;
            if len != b"1" {
                return Err(nom::Err::Failure((input, nom::error::ErrorKind::Tag)));
            }
            let (input, _) = crlf(input)?;
            Ok((input, Array { inner: None }))
        }
        Some(_) => {
            let (input, len) = digit1(input)?;
            let len = unsafe { std::str::from_utf8_unchecked(len).to_owned() };
            let len = len
                .parse::<usize>()
                .map_err(|_| nom::Err::Failure((input, nom::error::ErrorKind::Tag)))?;
            let (mut input, _) = crlf(input)?;
            let mut values = Vec::new();
            for _ in 0..len {
                let (i, value) = message(input)?;
                values.push(value);
                input = i;
            }
            Ok((
                input,
                Array {
                    inner: Some(values),
                },
            ))
        }
        None => Err(Err::Incomplete(Needed::Size(1))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse() {
        assert_eq!(message(b"$-1\r\n"), Ok((&b""[..], Message::null(),)));

        assert_eq!(
            message(b"$0\r\n\r\n"),
            Ok((&b""[..], Message::bulk_string(&[])))
        );

        assert_eq!(
            message(b"$1\r\n\0\r\n"),
            Ok((&b""[..], Message::bulk_string(&[0])))
        );

        assert_eq!(
            message(b"$11\r\nHELLO WORLD\r\n"),
            Ok((&b""[..], Message::bulk_string("HELLO WORLD".as_bytes())))
        );
    }
}
