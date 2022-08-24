// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use super::*;

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum SetMode {
    Add,
    Replace,
    Set,
}

#[derive(Debug, PartialEq, Eq)]
pub struct SetRequest {
    key: Box<[u8]>,
    value: Box<[u8]>,
    expire_time: Option<ExpireTime>,
    mode: SetMode,
    get_old: bool,
}

impl TryFrom<Message> for SetRequest {
    type Error = ParseError;

    fn try_from(other: Message) -> Result<Self, ParseError> {
        if let Message::Array(array) = other {
            if array.inner.is_none() {
                return Err(ParseError::Invalid);
            }

            let mut array = array.inner.unwrap();

            if array.len() < 3 {
                return Err(ParseError::Invalid);
            }

            let key = take_bulk_string(&mut array)?;
            if key.is_empty() {
                return Err(ParseError::Invalid);
            }

            let value = take_bulk_string(&mut array)?;

            let mut expire_time = None;
            let mut mode = SetMode::Set;
            let mut get_old = false;

            let mut i = 1;

            while i < array.len() {
                if let Message::BulkString(field) = &array[i] {
                    if field.inner.is_none() {
                        return Err(ParseError::Invalid);
                    }
                    let field = field.inner.as_ref().unwrap();

                    match field.as_ref() {
                        b"EX" => {
                            if expire_time.is_some() || array.len() < i + 2 {
                                return Err(ParseError::Invalid);
                            }
                            let s = take_bulk_string_as_u64(&mut array)?;
                            expire_time = Some(ExpireTime::Seconds(s));
                            i += 1;
                        }
                        b"PX" => {
                            if expire_time.is_some() || array.len() < i + 2 {
                                return Err(ParseError::Invalid);
                            }
                            let ms = take_bulk_string_as_u64(&mut array)?;
                            expire_time = Some(ExpireTime::Milliseconds(ms));
                            i += 1;
                        }
                        b"EXAT" => {
                            if expire_time.is_some() || array.len() < i + 2 {
                                return Err(ParseError::Invalid);
                            }
                            let s = take_bulk_string_as_u64(&mut array)?;
                            expire_time = Some(ExpireTime::UnixSeconds(s));
                            i += 1;
                        }
                        b"PXAT" => {
                            if expire_time.is_some() || array.len() < i + 2 {
                                return Err(ParseError::Invalid);
                            }
                            let ms = take_bulk_string_as_u64(&mut array)?;
                            expire_time = Some(ExpireTime::UnixMilliseconds(ms));
                            i += 1;
                        }
                        b"KEEPTTL" => {
                            if expire_time.is_some() {
                                return Err(ParseError::Invalid);
                            }
                            expire_time = Some(ExpireTime::KeepTtl);
                        }
                        b"NX" => {
                            if mode != SetMode::Set {
                                return Err(ParseError::Invalid);
                            }

                            mode = SetMode::Add;
                        }
                        b"XX" => {
                            if mode != SetMode::Set {
                                return Err(ParseError::Invalid);
                            }

                            mode = SetMode::Replace;
                        }
                        b"GET" => {
                            if get_old {
                                return Err(ParseError::Invalid);
                            }

                            get_old = true;
                        }
                        _ => {
                            return Err(ParseError::Invalid);
                        }
                    }
                } else {
                    return Err(ParseError::Invalid);
                }

                i += 1;
            }

            Ok(Self {
                key,
                value,
                expire_time,
                mode,
                get_old,
            })
        } else {
            Err(ParseError::Invalid)
        }
    }
}

impl SetRequest {
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

impl Compose for SetRequest {
    fn compose(&self, session: &mut session::Session) {
        let mut alen = 3;
        match self.expire_time {
            None => {}
            Some(ExpireTime::KeepTtl) => {
                alen += 1;
            }
            Some(_) => {
                alen += 2;
            }
        }
        if self.mode != SetMode::Set {
            alen += 1;
        }
        if self.get_old {
            alen += 1;
        }

        let _ = session
            .write_all(format!("*{}\r\n$3\r\nSET\r\n${}\r\n", alen, self.key.len()).as_bytes());
        let _ = session.write_all(&self.key);
        let _ = session.write_all(b"\r\n");
        let _ = session.write_all(format!("${}\r\n", self.value.len()).as_bytes());
        let _ = session.write_all(&self.value);
        let _ = session.write_all(b"\r\n");
        if let Some(expire_time) = self.expire_time {
            match expire_time {
                ExpireTime::Seconds(s) => {
                    let s = format!("{}", s);
                    let _ = session
                        .write_all(format!("$2\r\nEX\r\n${}\r\n{}\r\n", s.len(), s).as_bytes());
                }
                ExpireTime::Milliseconds(ms) => {
                    let ms = format!("{}", ms);
                    let _ = session
                        .write_all(format!("$2\r\nPX\r\n${}\r\n{}\r\n", ms.len(), ms).as_bytes());
                }
                ExpireTime::UnixSeconds(s) => {
                    let s = format!("{}", s);
                    let _ = session
                        .write_all(format!("$4\r\nEXAT\r\n${}\r\n{}\r\n", s.len(), s).as_bytes());
                }
                ExpireTime::UnixMilliseconds(ms) => {
                    let ms = format!("{}", ms);
                    let _ = session
                        .write_all(format!("$4\r\nPXAT\r\n${}\r\n{}\r\n", ms.len(), ms).as_bytes());
                }
                ExpireTime::KeepTtl => {
                    let _ = session.write_all(b"$7\r\nKEEPTTL\r\n");
                }
            }
        }
        match self.mode {
            SetMode::Add => {
                let _ = session.write_all(b"$2\r\nNX\r\n");
            }
            SetMode::Replace => {
                let _ = session.write_all(b"$2\r\nXX\r\n");
            }
            SetMode::Set => {}
        }
        if self.get_old {
            let _ = session.write_all(b"$3\r\nGET\r\n");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser() {
        let parser = RequestParser::new();
        if let Request::Set(request) = parser.parse(b"set 0 1\r\n").unwrap().into_inner() {
            assert_eq!(request.key(), b"0");
            assert_eq!(request.value(), b"1");
        } else {
            panic!("invalid parse result");
        }

        if let Request::Set(request) = parser
            .parse(b"*3\r\n$3\r\nset\r\n$1\r\n0\r\n$1\r\n1\r\n")
            .unwrap()
            .into_inner()
        {
            assert_eq!(request.key(), b"0");
            assert_eq!(request.value(), b"1");
        } else {
            panic!("invalid parse result");
        }
    }
}
