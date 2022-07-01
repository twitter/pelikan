// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use super::*;

const MSG: &[u8] = b"ERROR\r\n";

#[derive(Debug, PartialEq, Eq)]
pub struct Error {}

impl Error {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Default for Error {
    fn default() -> Self {
        Error {}
    }
}

impl SimpleResponse for Error {
    fn code(&self) -> u8 {
        0
    }

    fn is_empty(&self) -> bool {
        false
    }

    fn len(&self) -> usize {
        MSG.len()
    }
}

impl Compose for Error {
    fn compose(&self, session: &mut session::Session) {
        let _ = session.write_all(MSG);
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
