// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use super::*;
use std::ops::Deref;

#[derive(Debug, PartialEq, Eq)]
pub struct Gets {
    pub(crate) keys: Box<[Box<[u8]>]>,
}

impl Gets {
    pub fn keys(&self) -> &[Box<[u8]>] {
        self.keys.as_ref()
    }
}

impl RequestParser {
    // this is to be called after parsing the command, so we do not match the verb
    pub(crate) fn parse_gets<'a>(&self, input: &'a [u8]) -> IResult<&'a [u8], Gets> {
        // we can use the get parser here and convert the request
        match self.parse_get_no_stats(input) {
            Ok((input, request)) => {
                GETS.increment();
                let keys = request.keys.len() as u64;
                GETS_KEY.add(keys);
                Ok((input, Gets { keys: request.keys }))
            }
            Err(e) => {
                if !e.is_incomplete() {
                    GETS.increment();
                    GETS_EX.increment();
                }
                Err(e)
            }
        }
    }
}

impl Compose for Gets {
    fn compose(&self, session: &mut dyn BufMut) -> usize {
        let verb = b"gets";

        let mut size = verb.len() + CRLF.len();

        session.put_slice(verb);
        for key in self.keys.iter() {
            session.put_slice(b" ");
            session.put_slice(key);
            size += 1 + key.len();
        }
        session.put_slice(CRLF);

        size
    }
}

impl Klog for Gets {
    type Response = Response;

    fn klog(&self, response: &Self::Response) {
        if let Response::Values(ref res) = response {
            let total_keys = self.keys.len();
            let hit_keys = res.values.len();
            let miss_keys = total_keys - hit_keys;
            GETS_KEY_HIT.add(hit_keys as _);
            GETS_KEY_MISS.add(miss_keys as _);

            let values = res.values();
            let mut value_index = 0;

            for key in self.keys() {
                let key = key.deref();
                // if we are out of values or the keys don't match, it's a miss
                if value_index >= values.len() || values[value_index].key() != key {
                    klog!("\"get {}\" {} 0", String::from_utf8_lossy(key), MISS);
                } else {
                    klog!(
                        "\"get {}\" {} {}",
                        String::from_utf8_lossy(key),
                        HIT,
                        values[value_index].len()
                    );
                    value_index += 1;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse() {
        let parser = RequestParser::new();

        // test parsing a simple request
        assert_eq!(
            parser.parse_request(b"gets key \r\n"),
            Ok((
                &b""[..],
                Request::Gets(Gets {
                    keys: vec![b"key".to_vec().into_boxed_slice()].into_boxed_slice(),
                })
            ))
        );

        // command name is not case sensitive
        assert_eq!(
            parser.parse_request(b"gets key \r\n"),
            parser.parse_request(b"GETS key \r\n"),
        );

        // trailing spaces don't matter
        assert_eq!(
            parser.parse_request(b"gets key\r\n"),
            parser.parse_request(b"gets key \r\n"),
        );

        // multiple trailing spaces is fine too
        assert_eq!(
            parser.parse_request(b"gets key\r\n"),
            parser.parse_request(b"gets key      \r\n"),
        );

        // request can have multiple keys
        assert_eq!(
            parser.parse_request(b"gets a b c\r\n"),
            Ok((
                &b""[..],
                Request::Gets(Gets {
                    keys: vec![
                        b"a".to_vec().into_boxed_slice(),
                        b"b".to_vec().into_boxed_slice(),
                        b"c".to_vec().into_boxed_slice(),
                    ]
                    .into_boxed_slice(),
                })
            ))
        );

        // key is binary safe
        assert_eq!(
            parser.parse_request(b"gets evil\0key \r\n"),
            Ok((
                &b""[..],
                Request::Gets(Gets {
                    keys: vec![b"evil\0key".to_vec().into_boxed_slice(),].into_boxed_slice()
                })
            ))
        );
    }
}
