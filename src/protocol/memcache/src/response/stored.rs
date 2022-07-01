// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use super::*;

const MSG: &[u8] = b"STORED\r\n";

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
            let _ = session.write_all(MSG);
        }
    }
}

impl SimpleResponse for Stored {
    fn code(&self) -> u8 {
        5
    }

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn len(&self) -> usize {
        if self.noreply {
            0
        } else {
            MSG.len()
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
