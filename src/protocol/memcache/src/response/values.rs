// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use super::*;

#[derive(Debug, PartialEq, Eq)]
pub struct Values {
    pub(crate) values: Vec<Value>,
}

impl Values {
    pub fn new(values: Vec<Value>) -> Self {
        Self { values }
    }

    pub fn values(&self) -> &[Value] {
        &self.values
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Value {
    // data holds both key and value (if present)
    data: Vec<u8>,
    klen: usize,
    vlen: Option<usize>,

    flags: u32,
    cas: Option<u64>,
}

impl Value {
    pub fn new(key: &[u8], flags: u32, cas: Option<u64>, value: &[u8]) -> Self {
        Self::new_with_buffer(
            key,
            flags,
            cas,
            value,
            Vec::with_capacity(key.len() + value.len()),
        )
    }

    pub fn none(key: &[u8]) -> Self {
        Self::none_with_buffer(key, Vec::with_capacity(key.len()))
    }

    pub fn new_with_buffer(
        key: &[u8],
        flags: u32,
        cas: Option<u64>,
        value: &[u8],
        mut buf: Vec<u8>,
    ) -> Self {
        buf.clear();
        buf.reserve(key.len() + value.len());
        buf.extend_from_slice(key);
        buf.extend_from_slice(value);

        let klen = key.len();
        let vlen = Some(value.len());

        Self {
            data: buf,
            klen,
            vlen,

            flags,
            cas,
        }
    }

    pub fn none_with_buffer(key: &[u8], mut buf: Vec<u8>) -> Self {
        buf.clear();
        buf.extend_from_slice(key);

        Self {
            data: buf,
            klen: key.len(),
            vlen: None,
            flags: 0,
            cas: None,
        }
    }

    pub fn key(&self) -> &[u8] {
        &self.data[0..self.klen]
    }

    pub fn vlen(&self) -> Option<usize> {
        self.vlen
    }

    pub fn into_buf(mut self) -> Vec<u8> {
        self.data.clear();
        self.data
    }
}

impl Compose for Values {
    fn compose(&self, session: &mut dyn BufMut) -> usize {
        let suffix = b"END\r\n";

        let mut size = suffix.len();

        for value in self.values.iter() {
            size += value.compose(session);
        }
        session.put_slice(suffix);

        size
    }
}

impl Compose for Value {
    fn compose(&self, session: &mut dyn BufMut) -> usize {
        if self.vlen.is_none() {
            return 0;
        }

        let key = &self.data[0..self.klen];
        let value = &self.data[self.klen..];

        let prefix = b"VALUE ";
        let header_fields = if let Some(cas) = self.cas {
            format!(" {} {} {}\r\n", self.flags, value.len(), cas).into_bytes()
        } else {
            format!(" {} {}\r\n", self.flags, value.len()).into_bytes()
        };

        let size = prefix.len() + key.len() + header_fields.len() + value.len() + CRLF.len();

        session.put_slice(prefix);
        session.put_slice(key);
        session.put_slice(&header_fields);
        session.put_slice(value);
        session.put_slice(CRLF);

        size
    }
}

pub fn parse(input: &[u8]) -> IResult<&[u8], Values> {
    let mut values = Vec::new();
    let mut input = input;
    loop {
        let (i, _) = space1(input)?;
        let (i, key) = take_till(|b| (b == b' ' || b == b'\r'))(i)?;

        let (i, _) = space1(i)?;
        let (i, flags) = parse_u32(i)?;

        let (i, _) = space1(i)?;
        let (i, bytes) = parse_usize(i)?;

        input = i;

        let mut cas: Option<u64> = None;

        // if we have a space, we might have a cas value
        if let Ok((i, _)) = space1(input) {
            // we need to check to make sure we didn't stop because
            // of the CRLF
            let (i, c) = take_till(|b| b == b'\r')(i)?;
            if !c.is_empty() {
                // make sure it's a valid string
                let c = std::str::from_utf8(c)
                    .map_err(|_| nom::Err::Failure((i, nom::error::ErrorKind::Tag)))?;
                // and make sure that string represents a 64bit integer
                cas = Some(
                    c.parse::<u64>()
                        .map_err(|_| nom::Err::Failure((i, nom::error::ErrorKind::Tag)))?,
                );
            }
            input = i;
        }

        // we then have zero or more spaces until the CRLF
        let (i, _) = space0(input)?;
        let (i, _) = crlf(i)?;

        // we know how many bytes of value, and that its followed by a CRLF
        let (i, value) = take(bytes)(i)?;
        let (i, _) = crlf(i)?;

        // add to the collection of values
        values.push(Value::new(key, flags, cas, value));

        // look for a space or the start of a CRLF
        let (i, s) = take_till(|b| (b == b' ' || b == b'\r'))(i)?;

        // we should have found one of the following tokens
        match s {
            b"END" | b"end" => {
                // no more values as part of this response, consume the crlf
                // and stop processing
                let (i, _) = crlf(i)?;
                input = i;
                break;
            }
            b"VALUE" | b"value" => {
                // we have another value, loop again
                input = i;
                continue;
            }
            _ => {
                return Err(nom::Err::Failure((i, nom::error::ErrorKind::Tag)));
            }
        }
    }

    Ok((input, Values { values }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse() {
        // simple single value response
        let value_0 = Value::new(b"0", 0, None, b"1");
        assert_eq!(
            response(b"VALUE 0 0 1\r\n1\r\nEND\r\n"),
            Ok((&b""[..], Response::values(vec![value_0.clone()]),))
        );

        // binary data for the value
        let value_1 = Value::new(b"1", 1, None, b"\0");
        assert_eq!(
            response(b"VALUE 1 1 1\r\n\0\r\nEND\r\n"),
            Ok((&b""[..], Response::values(vec![value_1.clone()]),))
        );

        // two values in the same response
        assert_eq!(
            response(b"VALUE 0 0 1\r\n1\r\nVALUE 1 1 1\r\n\0\r\nEND\r\n"),
            Ok((&b""[..], Response::values(vec![value_0, value_1]),))
        );

        // a value with zero-length data and a cas value
        let value_2 = Value::new(b"2", 100, Some(42), b"");
        assert_eq!(
            response(b"VALUE 2 100 0 42\r\n\r\nEND\r\n"),
            Ok((&b""[..], Response::values(vec![value_2]),))
        );

        // empty values response
        assert_eq!(
            response(b"END\r\n"),
            Ok((&b""[..], Response::values(vec![]),))
        );
    }
}
