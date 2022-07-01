// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use super::*;

#[derive(Debug, PartialEq, Eq)]
pub struct Quit {}

impl Quit {}

impl Display for Quit {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "quit")
    }
}

impl RequestParser {
    // this is to be called after parsing the command, so we do not match the verb
    pub fn parse_quit<'a>(&self, input: &'a [u8]) -> IResult<&'a [u8], Quit> {
        let (input, _) = space0(input)?;
        let (input, _) = crlf(input)?;

        Ok((input, Quit {}))
    }
}

impl Compose for Quit {
    fn compose(&self, session: &mut session::Session) {
        let _ = session.write_all(b"quit\r\n");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse() {
        let parser = RequestParser::new();

        // quit command
        assert_eq!(
            parser.parse_request(b"quit\r\n"),
            Ok((&b""[..], Request::Quit(Quit {})))
        );
    }
}
