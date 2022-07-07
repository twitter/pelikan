// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use super::*;

#[derive(Debug, PartialEq, Eq)]
pub struct Numeric {
    value: u64,
    noreply: bool,
}

impl Numeric {
    pub fn new(value: u64, noreply: bool) -> Self {
        Self { value, noreply }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn len(&self) -> usize {
        if self.noreply {
            0
        } else {
            format!("{}\r\n", self.value).len()
        }
    }
}

impl Compose for Numeric {
    fn compose(&self, session: &mut Session) {
        if !self.noreply {
            let _ = session.write_all(format!("{}\r\n", self.value).as_bytes());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse() {
        assert_eq!(
            response(b"0\r\n"),
            Ok((&b""[..], Response::numeric(0, false),))
        );

        assert_eq!(
            response(b"42 \r\n"),
            Ok((&b""[..], Response::numeric(42, false),))
        );
    }
}
