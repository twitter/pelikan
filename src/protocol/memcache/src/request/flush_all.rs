// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use super::*;

#[derive(Debug, PartialEq, Eq)]
pub struct FlushAll {
    delay: u32,
    noreply: bool,
}

impl FlushAll {
    pub fn delay(&self) -> u32 {
        self.delay
    }

    pub fn noreply(&self) -> bool {
        self.noreply
    }
}

impl RequestParser {
    // this is to be called after parsing the command, so we do not match the verb
    pub fn parse_flush_all_no_stats<'a>(&self, input: &'a [u8]) -> IResult<&'a [u8], FlushAll> {
        let mut input = input;

        let mut delay = 0;
        let mut noreply = false;

        // if we have a space, we might have a delay value
        if let Ok((i, _)) = space1(input) {
            // we need to check to make sure we didn't stop because
            // of the CRLF
            let (i, c) = take_till(|b| (b == b' ' || b == b'\r'))(i)?;
            if !c.is_empty() {
                // make sure it's a valid string
                let c = std::str::from_utf8(c)
                    .map_err(|_| nom::Err::Failure((i, nom::error::ErrorKind::Tag)))?;

                if c == "noreply" {
                    noreply = true;
                } else {
                    // and make sure that sring represents a 64bit integer
                    delay = c
                        .parse::<u32>()
                        .map_err(|_| nom::Err::Failure((i, nom::error::ErrorKind::Tag)))?;
                }
            }
            input = i;
        }

        // if we have a space, we might have a noreply
        if let Ok((i, _)) = space1(input) {
            let (i, _) = tag(b"noreply")(i)?;
            noreply = true;
            input = i;
        }

        let (input, _) = space0(input)?;
        let (input, _) = crlf(input)?;

        Ok((input, FlushAll { delay, noreply }))
    }

    // this is to be called after parsing the command, so we do not match the verb
    pub fn parse_flush_all<'a>(&self, input: &'a [u8]) -> IResult<&'a [u8], FlushAll> {
        match self.parse_flush_all_no_stats(input) {
            Ok((input, request)) => {
                FLUSH_ALL.increment();
                Ok((input, request))
            }
            Err(e) => {
                if !e.is_incomplete() {
                    FLUSH_ALL.increment();
                    FLUSH_ALL_EX.increment();
                }
                Err(e)
            }
        }
    }
}

impl Compose for FlushAll {
    fn compose(&self, session: &mut dyn BufMut) -> usize {
        let verb = b"flush_all";
        let delay = if self.delay != 0 {
            format!(" {}", self.delay).into_bytes()
        } else {
            vec![]
        };
        let header_end = if self.noreply {
            " noreply\r\n".as_bytes()
        } else {
            "\r\n".as_bytes()
        };

        let size = verb.len() + delay.len() + header_end.len();

        session.put_slice(verb);
        session.put_slice(&delay);
        session.put_slice(header_end);

        size
    }
}

impl Klog for FlushAll {
    type Response = Response;

    fn klog(&self, _response: &Self::Response) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse() {
        let parser = RequestParser::new();

        // basic flush_all command
        assert_eq!(
            parser.parse_request(b"flush_all\r\n"),
            Ok((
                &b""[..],
                Request::FlushAll(FlushAll {
                    delay: 0,
                    noreply: false,
                })
            ))
        );

        // noreply
        assert_eq!(
            parser.parse_request(b"flush_all noreply\r\n"),
            Ok((
                &b""[..],
                Request::FlushAll(FlushAll {
                    delay: 0,
                    noreply: true,
                })
            ))
        );

        // delay
        assert_eq!(
            parser.parse_request(b"flush_all 42\r\n"),
            Ok((
                &b""[..],
                Request::FlushAll(FlushAll {
                    delay: 42,
                    noreply: false,
                })
            ))
        );

        // delay and noreply
        assert_eq!(
            parser.parse_request(b"flush_all 42 noreply\r\n"),
            Ok((
                &b""[..],
                Request::FlushAll(FlushAll {
                    delay: 42,
                    noreply: true,
                })
            ))
        );
    }
}
