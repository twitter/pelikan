// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use super::*;

#[derive(Debug, PartialEq, Eq)]
pub struct Integer {
    pub(crate) inner: u64,
}

impl Compose for Integer {
    fn compose(&self, session: &mut session::Session) {
        let _ = session.write_all(format!(":{}\r\n", self.inner).as_bytes());
    }
}

pub fn parse(input: &[u8]) -> IResult<&[u8], Integer> {
    let (input, string) = digit1(input)?;
    let (input, _) = crlf(input)?;

    let string = unsafe { std::str::from_utf8_unchecked(string).to_owned() };
    let value = string
        .parse::<u64>()
        .map_err(|_| nom::Err::Failure((input, nom::error::ErrorKind::Tag)))?;
    Ok((input, Integer { inner: value }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse() {
        assert_eq!(response(b":0\r\n"), Ok((&b""[..], Response::integer(0),)));

        assert_eq!(
            response(b":1000\r\n"),
            Ok((&b""[..], Response::integer(1000),))
        );
    }
}
