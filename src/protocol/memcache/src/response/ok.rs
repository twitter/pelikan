// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use super::*;

#[derive(Debug, PartialEq, Eq)]
pub struct Ok {}

impl Compose for Ok {
    fn compose(&self, session: &mut session::Session) {
        let _ = session.write_all(b"OK\r\n");
    }
}

pub fn parse(input: &[u8]) -> IResult<&[u8], Ok> {
    let (input, _) = space0(input)?;
    let (input, _) = crlf(input)?;
    Ok((input, Ok {}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse() {
        assert_eq!(response(b"OK\r\n"), Ok((&b""[..], Response::ok(),)));

        assert_eq!(response(b"OK \r\n"), Ok((&b""[..], Response::ok(),)));
    }
}
