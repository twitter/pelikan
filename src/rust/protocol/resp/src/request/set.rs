// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use super::*;

pub const MAX_OPTION_LEN: usize = 128;

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum SetMode {
    Add,
    Replace,
    Set,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Set {
    key: Box<[u8]>,
    value: Box<[u8]>,
    expire_time: Option<ExpireTime>,
    mode: SetMode,
    get_old: bool,
}

impl Set {
    pub fn key(&self) -> &[u8] {
        &self.key
    }

    pub fn value(&self) -> &[u8] {
        &self.value
    }

    pub fn expire_time(&self) -> Option<ExpireTime> {
        self.expire_time
    }

    pub fn mode(&self) -> SetMode {
        self.mode
    }

    pub fn get_old(&self) -> bool {
        self.get_old
    }
}

impl RequestParser {
    // this is to be called after parsing the command, so we do not match the verb
    pub(crate) fn parse_set_no_stats<'a>(&self, input: &'a [u8]) -> IResult<&'a [u8], Set> {
        let (input, _) = space1(input)?;
        let (input, key) = string(input, self.max_key_len)?;
        let (input, _) = space1(input)?;
        let (input, value) = string(input, self.max_value_size)?;
        let (mut input, _) = space0(input)?;

        let mut expire_time = None;
        let mut mode = SetMode::Set;
        let mut get_old = false;

        loop {
            match string(input, MAX_OPTION_LEN) {
                Ok((i, b"EX")) => {
                    if expire_time.is_some() {
                        return Err(nom::Err::Failure((i, nom::error::ErrorKind::Tag)));
                    }

                    let (i, _) = space1(i)?;

                    let (i, seconds) = parse_u64(i)?;

                    expire_time = Some(ExpireTime::Seconds(seconds));

                    let (i, _) = space0(i)?;

                    input = i;
                }
                Ok((i, b"PX")) => {
                    if expire_time.is_some() {
                        return Err(nom::Err::Failure((i, nom::error::ErrorKind::Tag)));
                    }

                    let (i, _) = space1(i)?;

                    let (i, milliseconds) = parse_u64(i)?;

                    expire_time = Some(ExpireTime::Milliseconds(milliseconds));

                    let (i, _) = space0(i)?;

                    input = i;
                }
                Ok((i, b"EXAT")) => {
                    if expire_time.is_some() {
                        return Err(nom::Err::Failure((i, nom::error::ErrorKind::Tag)));
                    }

                    let (i, _) = space1(i)?;

                    let (i, unix_seconds) = parse_u64(i)?;

                    expire_time = Some(ExpireTime::UnixSeconds(unix_seconds));

                    let (i, _) = space0(i)?;

                    input = i;
                }
                Ok((i, b"PXAT")) => {
                    if expire_time.is_some() {
                        return Err(nom::Err::Failure((i, nom::error::ErrorKind::Tag)));
                    }

                    let (i, _) = space1(i)?;

                    let (i, unix_milliseconds) = parse_u64(i)?;

                    expire_time = Some(ExpireTime::UnixMilliseconds(unix_milliseconds));

                    let (i, _) = space0(i)?;

                    input = i;
                }
                Ok((i, b"KEEPTTL")) => {
                    if expire_time.is_some() {
                        return Err(nom::Err::Failure((i, nom::error::ErrorKind::Tag)));
                    }

                    expire_time = Some(ExpireTime::KeepTtl);

                    let (i, _) = space0(i)?;

                    input = i;
                }
                Ok((i, b"NX")) => {
                    if mode != SetMode::Set {
                        return Err(nom::Err::Failure((i, nom::error::ErrorKind::Tag)));
                    }

                    mode = SetMode::Add;

                    let (i, _) = space0(i)?;

                    input = i;
                }
                Ok((i, b"XX")) => {
                    if mode != SetMode::Set {
                        return Err(nom::Err::Failure((i, nom::error::ErrorKind::Tag)));
                    }

                    mode = SetMode::Replace;

                    let (i, _) = space0(i)?;

                    input = i;
                }
                Ok((i, b"GET")) => {
                    if get_old {
                        return Err(nom::Err::Failure((i, nom::error::ErrorKind::Tag)));
                    }

                    get_old = true;

                    let (i, _) = space0(i)?;

                    input = i;
                }
                Ok((_, b"")) => {
                    break;
                }
                Ok((i, _)) => {
                    return Err(nom::Err::Failure((i, nom::error::ErrorKind::Tag)));
                }
                Err(_) => {
                    break;
                }
            }
        }

        let (input, _) = crlf(input)?;

        Ok((
            input,
            Set {
                key: key.to_owned().into_boxed_slice(),
                value: value.to_owned().into_boxed_slice(),
                expire_time,
                mode,
                get_old,
            },
        ))
    }

    pub fn parse_set<'a>(&self, input: &'a [u8]) -> IResult<&'a [u8], Set> {
        match self.parse_set_no_stats(input) {
            Ok((input, request)) => {
                SET.increment();
                Ok((input, request))
            }
            Err(e) => {
                if !e.is_incomplete() {
                    SET.increment();
                    SET_EX.increment();
                }
                Err(e)
            }
        }
    }
}

impl Compose for Set {
    fn compose(&self, session: &mut session::Session) {
        let _ = session.write_all(b"SET \"");
        let _ = session.write_all(&self.key);
        let _ = session.write_all(b"\" \"");
        let _ = session.write_all(&self.value);
        if let Some(expire_time) = self.expire_time {
            match expire_time {
                ExpireTime::Seconds(s) => {
                    let _ = session.write_all(format!(" EX {}", s).as_bytes());
                }
                ExpireTime::Milliseconds(ms) => {
                    let _ = session.write_all(format!(" PX {}", ms).as_bytes());
                }
                ExpireTime::UnixSeconds(s) => {
                    let _ = session.write_all(format!(" EXAT {}", s).as_bytes());
                }
                ExpireTime::UnixMilliseconds(ms) => {
                    let _ = session.write_all(format!(" PXAT {}", ms).as_bytes());
                }
                ExpireTime::KeepTtl => {
                    let _ = session.write_all(b" KEEPTTL");
                }
            }
        }
        match self.mode {
            SetMode::Add => {
                let _ = session.write_all(b" NX");
            }
            SetMode::Replace => {
                let _ = session.write_all(b" XX");
            }
            SetMode::Set => {}
        }
        if self.get_old {
            let _ = session.write_all(b" GET");
        }
        let _ = session.write_all(b"\r\n");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse() {
        let parser = RequestParser::new();

        // test parsing the command verb
        assert_eq!(
            command(b"set key value\r\n"),
            Ok((&b" key value\r\n"[..], Command::Set))
        );

        // test parsing the remainder of the request
        assert_eq!(
            parser.parse_set(b" key value\r\n"),
            Ok((
                &b""[..],
                Set {
                    key: b"key".to_vec().into_boxed_slice(),
                    value: b"value".to_vec().into_boxed_slice(),
                    expire_time: None,
                    mode: SetMode::Set,
                    get_old: false,
                }
            ))
        );

        // test parsing the entire request in one pass
        assert_eq!(
            parser.parse_request(b"set key value\r\n"),
            Ok((
                &b""[..],
                Request::Set(Set {
                    key: b"key".to_vec().into_boxed_slice(),
                    value: b"value".to_vec().into_boxed_slice(),
                    expire_time: None,
                    mode: SetMode::Set,
                    get_old: false,
                })
            ))
        );

        // test parsing with expire time in seconds
        assert_eq!(
            parser.parse_request(b"set key value EX 300\r\n"),
            Ok((
                &b""[..],
                Request::Set(Set {
                    key: b"key".to_vec().into_boxed_slice(),
                    value: b"value".to_vec().into_boxed_slice(),
                    expire_time: Some(ExpireTime::Seconds(300)),
                    mode: SetMode::Set,
                    get_old: false,
                })
            ))
        );

        // test parsing with expire time in milliseconds
        assert_eq!(
            parser.parse_request(b"set key value PX 12345\r\n"),
            Ok((
                &b""[..],
                Request::Set(Set {
                    key: b"key".to_vec().into_boxed_slice(),
                    value: b"value".to_vec().into_boxed_slice(),
                    expire_time: Some(ExpireTime::Milliseconds(12345)),
                    mode: SetMode::Set,
                    get_old: false,
                })
            ))
        );

        // test parsing with expire time in unix seconds
        assert_eq!(
            parser.parse_request(b"set key value EXAT 1652370171\r\n"),
            Ok((
                &b""[..],
                Request::Set(Set {
                    key: b"key".to_vec().into_boxed_slice(),
                    value: b"value".to_vec().into_boxed_slice(),
                    expire_time: Some(ExpireTime::UnixSeconds(1652370171)),
                    mode: SetMode::Set,
                    get_old: false,
                })
            ))
        );

        // test parsing with expire time in unix milliseconds
        assert_eq!(
            parser.parse_request(b"set key value PXAT 1652370171000\r\n"),
            Ok((
                &b""[..],
                Request::Set(Set {
                    key: b"key".to_vec().into_boxed_slice(),
                    value: b"value".to_vec().into_boxed_slice(),
                    expire_time: Some(ExpireTime::UnixMilliseconds(1652370171000)),
                    mode: SetMode::Set,
                    get_old: false,
                })
            ))
        );

        // test parsing with `KEEPTTL`
        assert_eq!(
            parser.parse_request(b"set key value KEEPTTL\r\n"),
            Ok((
                &b""[..],
                Request::Set(Set {
                    key: b"key".to_vec().into_boxed_slice(),
                    value: b"value".to_vec().into_boxed_slice(),
                    expire_time: Some(ExpireTime::KeepTtl),
                    mode: SetMode::Set,
                    get_old: false,
                })
            ))
        );

        // test parsing of a add
        assert_eq!(
            parser.parse_request(b"set key value NX\r\n"),
            Ok((
                &b""[..],
                Request::Set(Set {
                    key: b"key".to_vec().into_boxed_slice(),
                    value: b"value".to_vec().into_boxed_slice(),
                    expire_time: None,
                    mode: SetMode::Add,
                    get_old: false,
                })
            ))
        );

        // test parsing of a replace
        assert_eq!(
            parser.parse_request(b"set key value XX\r\n"),
            Ok((
                &b""[..],
                Request::Set(Set {
                    key: b"key".to_vec().into_boxed_slice(),
                    value: b"value".to_vec().into_boxed_slice(),
                    expire_time: None,
                    mode: SetMode::Replace,
                    get_old: false,
                })
            ))
        );

        // test parsing of a set that returns the previous value
        assert_eq!(
            parser.parse_request(b"set key value GET\r\n"),
            Ok((
                &b""[..],
                Request::Set(Set {
                    key: b"key".to_vec().into_boxed_slice(),
                    value: b"value".to_vec().into_boxed_slice(),
                    expire_time: None,
                    mode: SetMode::Set,
                    get_old: true,
                })
            ))
        );
    }
}
