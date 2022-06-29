// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use super::*;

#[derive(Debug, PartialEq, Eq)]
pub struct Stored {
    noreply: bool,
}

impl Stored {
    pub fn new(noreply: bool) -> Self {
        Self { noreply }
    }
}

impl Compose for Stored {
    fn compose(&self, session: &mut session::Session) {
        if !self.noreply {
            let _ = session.write_all(b"STORED\r\n");
        }
    }
}

pub fn parse(input: &[u8]) -> IResult<&[u8], Stored> {
    let (input, _) = space0(input)?;
    let (input, _) = crlf(input)?;
    Ok((input, Stored { noreply: false }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse() {
        assert_eq!(
            response(b"STORED\r\n"),
            Ok((&b""[..], Response::stored(false),))
        );

        assert_eq!(
            response(b"STORED \r\n"),
            Ok((&b""[..], Response::stored(false),))
        );
    }
}
