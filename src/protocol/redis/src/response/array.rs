// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use super::*;
use protocol_common::Compose;

#[derive(Debug, PartialEq, Eq)]
pub struct Array {
    pub(crate) inner: Option<Vec<Response>>,
}

impl Compose for Array {
    fn compose(&self, session: &mut session::Session) {
        if let Some(values) = &self.inner {
            let _ = session.write_all(format!("${}\r\n", values.len()).as_bytes());
            for value in values {
                value.compose(session);
            }
            let _ = session.write_all(b"\r\n");
        } else {
            let _ = session.write_all(b"*-1\r\n");
        }
    }
}

pub fn parse(input: &[u8]) -> IResult<&[u8], Array> {
    match input.get(0) {
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
                let (i, value) = response(input)?;
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
        assert_eq!(response(b"$-1\r\n"), Ok((&b""[..], Response::null(),)));

        assert_eq!(
            response(b"$0\r\n\r\n"),
            Ok((&b""[..], Response::bulk_string(&[])))
        );

        assert_eq!(
            response(b"$1\r\n\0\r\n"),
            Ok((&b""[..], Response::bulk_string(&[0])))
        );

        assert_eq!(
            response(b"$11\r\nHELLO WORLD\r\n"),
            Ok((&b""[..], Response::bulk_string("HELLO WORLD".as_bytes())))
        );
    }
}
