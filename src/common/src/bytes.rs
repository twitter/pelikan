// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

pub trait SliceExtension {
    ///Trims a type by skipping contiguous whitespace from the beginning and end
    fn trim(&self) -> &Self;
}

impl SliceExtension for [u8] {
    ///Trims an &[u8] by skipping contiguous whitespace from the beginning and end
    fn trim(&self) -> &[u8] {
        let first = self.iter().position(|c| !c.is_ascii_whitespace());
        let last = self.iter().rposition(|c| !c.is_ascii_whitespace());
        match (first, last) {
            (Some(first), Some(last)) => &self[first..=last],
            _ => Default::default(),
        }
    }
}

#[cfg(test)]
mod test {
    use crate::bytes::SliceExtension;

    #[test]
    fn it_can_trim_byteslice() {
        assert_eq!(b" foobar ".trim(), b"foobar");
        assert_eq!(b"\nfoobar\n ".trim(), b"foobar");
        assert_eq!(b"foo bar".trim(), b"foo bar");
        assert_eq!(b"foobar".trim(), b"foobar");
        assert_eq!(b"".trim(), b"");
        assert_eq!(b" ".trim(), b"");
        assert_eq!(b"    ".trim(), b"");
        assert_eq!(b"  \n\t ".trim(), b"");
    }
}
