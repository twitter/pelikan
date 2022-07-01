// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use super::*;

const MSG: &[u8] = b"NOT_FOUND\r\n";

#[derive(Debug, PartialEq, Eq)]
pub struct NotFound {
    noreply: bool,
}

impl NotFound {
    pub fn new(noreply: bool) -> Self {
        Self { noreply }
    }
}

impl Compose for NotFound {
    fn compose(&self, session: &mut session::Session) {
        if !self.noreply {
            let _ = session.write_all(MSG);
        }
    }
}

impl SimpleResponse for NotFound {
    fn code(&self) -> u8 {
        8
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

pub fn parse(input: &[u8]) -> IResult<&[u8], NotFound> {
    let (input, _) = space0(input)?;
    let (input, _) = crlf(input)?;
    Ok((input, NotFound { noreply: false }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse() {
        assert_eq!(
            response(b"NOT_FOUND\r\n"),
            Ok((&b""[..], Response::not_found(false),))
        );

        assert_eq!(
            response(b"NOT_FOUND \r\n"),
            Ok((&b""[..], Response::not_found(false),))
        );
    }
}