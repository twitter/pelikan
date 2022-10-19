// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use super::*;

#[derive(Debug, PartialEq, Eq)]
pub struct Error {
    pub(crate) inner: String,
}

impl Compose for Error {
    fn compose(&self, buf: &mut dyn BufMut) -> usize {
        buf.put_slice(b"-");
        buf.put_slice(self.inner.as_bytes());
        buf.put_slice(b"\r\n");
        self.inner.as_bytes().len() + 3
    }
}

pub fn parse(input: &[u8]) -> IResult<&[u8], Error> {
    let (input, string) = not_line_ending(input)?;
    let (input, _) = crlf(input)?;
    Ok((
        input,
        Error {
            inner: unsafe { std::str::from_utf8_unchecked(string).to_owned() },
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse() {
        assert_eq!(
            message(b"-Error message\r\n"),
            Ok((&b""[..], Message::error("Error message"),))
        );

        assert_eq!(
            message(b"-ERR unknown command 'helloworld'\r\n"),
            Ok((&b""[..], Message::error("ERR unknown command 'helloworld'"),))
        );
    }
}
