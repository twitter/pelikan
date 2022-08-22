// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use super::*;

#[derive(Debug, PartialEq, Eq)]
pub struct BulkString {
    pub(crate) inner: Option<Box<[u8]>>,
}

impl Compose for BulkString {
    fn compose(&self, session: &mut session::Session) {
        if let Some(value) = &self.inner {
            let _ = session.write_all(format!("${}\r\n", value.len()).as_bytes());
            let _ = session.write_all(&value);
            let _ = session.write_all(b"\r\n");
        } else {
            let _ = session.write_all(b"$-1\r\n");
        }
    }
}

pub fn parse(input: &[u8]) -> IResult<&[u8], BulkString> {
    match input.get(0) {
        Some(b'-') => {
            let (input, _) = take(1usize)(input)?;
            let (input, len) = digit1(input)?;
            if len != b"1" {
                return Err(nom::Err::Failure((input, nom::error::ErrorKind::Tag)));
            }
            let (input, _) = crlf(input)?;
            Ok((input, BulkString { inner: None }))
        }
        Some(_) => {
            let (input, len) = digit1(input)?;
            let len = unsafe { std::str::from_utf8_unchecked(len).to_owned() };
            let len = len
                .parse::<usize>()
                .map_err(|_| nom::Err::Failure((input, nom::error::ErrorKind::Tag)))?;
            let (input, _) = crlf(input)?;
            let (input, value) = take(len)(input)?;
            let (input, _) = crlf(input)?;
            Ok((
                input,
                BulkString {
                    inner: Some(value.to_vec().into_boxed_slice()),
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