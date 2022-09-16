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

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn len(&self) -> usize {
        if self.noreply {
            0
        } else {
            MSG.len()
        }
    }
}

impl Compose for Stored {
    fn compose(&self, session: &mut dyn BufMut) -> usize {
        if !self.noreply {
            session.put_slice(MSG);
            MSG.len()
        } else {
            0
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
