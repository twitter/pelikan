// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use super::*;

#[derive(Debug, PartialEq, Eq)]
pub struct Error {}

impl Error {
    pub fn new() -> Self {
        Self { }
    }
}

impl Compose for Error {
    fn compose(&self, session: &mut session::Session) {
        let _ = session.write_all(b"ERROR\r\n");
    }
}

pub fn parse(input: &[u8]) -> IResult<&[u8], Error> {
    let (input, _) = space0(input)?;
    let (input, _) = crlf(input)?;
    Ok((input, Error {}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse() {
        assert_eq!(response(b"ERROR\r\n"), Ok((&b""[..], Response::error(),)));

        assert_eq!(response(b"ERROR \r\n"), Ok((&b""[..], Response::error(),)));
    }
}
