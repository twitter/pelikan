// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use super::*;

const MSG: &[u8] = b"DELETED\r\n";

#[derive(Debug, PartialEq, Eq)]
pub struct Deleted {
    noreply: bool,
}

impl Deleted {
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

impl Compose for Deleted {
    fn compose(&self, session: &mut dyn BufMut) {
        if !self.noreply {
            session.put_slice(MSG);
        }
    }
}

pub fn parse(input: &[u8]) -> IResult<&[u8], Deleted> {
    let (input, _) = space0(input)?;
    let (input, _) = crlf(input)?;
    Ok((input, Deleted { noreply: false }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse() {
        assert_eq!(
            response(b"DELETED\r\n"),
            Ok((&b""[..], Response::deleted(false),))
        );

        assert_eq!(
            response(b"DELETED \r\n"),
            Ok((&b""[..], Response::deleted(false),))
        );
    }
}
