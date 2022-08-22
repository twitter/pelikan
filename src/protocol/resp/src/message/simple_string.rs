// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use super::*;

#[derive(Debug, PartialEq, Eq)]
pub struct SimpleString {
    pub(crate) inner: String,
}

impl Compose for SimpleString {
    fn compose(&self, session: &mut session::Session) {
        let _ = session.write_all(b"+");
        let _ = session.write_all(self.inner.as_bytes());
        let _ = session.write_all(b"\r\n");
    }
}

pub fn parse(input: &[u8]) -> IResult<&[u8], SimpleString> {
    let (input, string) = not_line_ending(input)?;
    let (input, _) = crlf(input)?;
    Ok((
        input,
        SimpleString {
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
            message(b"+OK\r\n"),
            Ok((&b""[..], Message::simple_string("OK"),))
        );

        assert_eq!(
            message(b"+SOME STRING WITH SPACES\r\n"),
            Ok((&b""[..], Message::simple_string("SOME STRING WITH SPACES")))
        );
    }
}
