// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use super::*;

#[derive(Debug, PartialEq, Eq)]
pub struct Error {
    pub(crate) inner: String,
}

impl Compose for Error {
    fn compose(&self, session: &mut session::Session) {
        let _ = session.write_all(b"-");
        let _ = session.write_all(self.inner.as_bytes());
        let _ = session.write_all(b"\r\n");
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
            response(b"-Error message\r\n"),
            Ok((&b""[..], Response::error("Error message"),))
        );

        assert_eq!(
            response(b"-ERR unknown command 'helloworld'\r\n"),
            Ok((
                &b""[..],
                Response::error("ERR unknown command 'helloworld'"),
            ))
        );
    }
}
