// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use super::*;

#[derive(Debug, PartialEq, Eq)]
pub struct Integer {
    pub(crate) inner: i64,
}

impl Compose for Integer {
    fn compose(&self, buf: &mut dyn BufMut) -> usize {
        let data = format!(":{}\r\n", self.inner);
        let _ = buf.put_slice(data.as_bytes());
        data.as_bytes().len()
    }
}

pub fn parse(input: &[u8]) -> IResult<&[u8], Integer> {
    let (input, string) = digit1(input)?;
    let (input, _) = crlf(input)?;

    let string = unsafe { std::str::from_utf8_unchecked(string).to_owned() };
    let value = string
        .parse::<i64>()
        .map_err(|_| nom::Err::Failure((input, nom::error::ErrorKind::Tag)))?;
    Ok((input, Integer { inner: value }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse() {
        assert_eq!(message(b":0\r\n"), Ok((&b""[..], Message::integer(0),)));

        assert_eq!(
            message(b":1000\r\n"),
            Ok((&b""[..], Message::integer(1000),))
        );
    }
}
