// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use super::*;

const MSG_PREFIX: &[u8] = b"CLIENT_ERROR ";

#[derive(Debug, PartialEq, Eq)]
pub struct ClientError {
    pub(crate) inner: String,
}

impl ClientError {
    pub fn is_empty(&self) -> bool {
        false
    }

    pub fn len(&self) -> usize {
        MSG_PREFIX.len() + self.inner.len() + 2
    }
}

impl Compose for ClientError {
    fn compose(&self, session: &mut dyn BufMut) {
        session.put_slice(MSG_PREFIX);
        session.put_slice(self.inner.as_bytes());
        session.put_slice(b"\r\n");
    }
}

pub fn parse(input: &[u8]) -> IResult<&[u8], ClientError> {
    let (input, _) = space0(input)?;
    let (input, string) = not_line_ending(input)?;
    let (input, _) = crlf(input)?;
    Ok((
        input,
        ClientError {
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
            response(b"CLIENT_ERROR Error message\r\n"),
            Ok((&b""[..], Response::client_error("Error message"),))
        );

        assert_eq!(
            response(b"CLIENT_ERROR\r\n"),
            Ok((&b""[..], Response::client_error(""),))
        );
    }
}
