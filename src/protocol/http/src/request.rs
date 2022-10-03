// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use std::mem::MaybeUninit;

use crate::{Error, ParseResult};
use httparse::{Header, ParserConfig, Status};
use protocol_common::{Parse, ParseOk};

pub struct Headers(Vec<(String, Vec<u8>)>);

pub struct ParseData(pub Result<Request, Error>);

impl Headers {
    fn from_httparse(headers: &[Header]) -> Self {
        Self(
            headers
                .iter()
                .map(|hdr| (hdr.name.to_owned(), hdr.value.to_owned()))
                .collect(),
        )
    }

    pub fn header(&self, hdr: &str) -> Option<&[u8]> {
        self.0
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(hdr))
            .map(|(_, value)| &**value)
    }
}

pub struct Request {
    pub data: RequestData,
    pub headers: Headers,
}

impl Request {
    pub fn data(&self) -> &RequestData {
        &self.data
    }

    pub fn header(&self, hdr: &str) -> Option<&[u8]> {
        self.headers.header(hdr)
    }
}

pub enum RequestData {
    Get(Vec<u8>),
    Put(Vec<u8>, Vec<u8>),
    Delete(Vec<u8>),
}

#[derive(Clone)]
pub struct RequestParser {
    config: ParserConfig,
}

impl RequestParser {
    pub fn new() -> Self {
        Self {
            config: ParserConfig::default(),
        }
    }

    fn do_parse(&self, buf: &mut &[u8]) -> ParseResult {
        let mut headers = [MaybeUninit::uninit(); 32];
        let mut request = httparse::Request::new(&mut []);
        let status =
            self.config
                .parse_request_with_uninit_headers(&mut request, *buf, &mut headers)?;

        let count = match status {
            Status::Complete(count) => count,
            Status::Partial => return Err(Error::PartialRequest(None)),
        };

        *buf = &buf[count..];

        let method = request.method.ok_or(Error::InternalError(
            "request was complete but had no method",
        ))?;
        let key = request
            .path
            .ok_or(Error::InternalError("request was complete but had no path"))?;

        let key = urlencoding::decode_binary(&key.as_bytes()).into_owned();
        let headers = Headers::from_httparse(request.headers);

        match method {
            "GET" => Ok(Request {
                data: RequestData::Get(key),
                headers,
            }),
            "DELETE" => Ok(Request {
                data: RequestData::Delete(key),
                headers,
            }),
            "PUT" => {
                let content_length = headers
                    .header("Content-Length")
                    .ok_or(Error::BadContentLength)?;
                let len: usize = std::str::from_utf8(content_length)
                    .map_err(|_| Error::BadContentLength)?
                    .parse()
                    .map_err(|_| Error::BadContentLength)?;

                if buf.len() < len {
                    return Err(Error::PartialRequest(Some(len - buf.len())));
                }

                let (value, newbuf) = buf.split_at(len);
                *buf = newbuf;

                Ok(Request {
                    data: RequestData::Put(key, value.to_owned()),
                    headers,
                })
            }
            _ => return Err(Error::BadRequestMethod),
        }
    }
}

impl Parse<ParseData> for RequestParser {
    fn parse(&self, buffer: &[u8]) -> Result<ParseOk<ParseData>, std::io::Error> {
        let mut buf = buffer;
        let result = self.do_parse(&mut buf);

        let consumed = match result.is_ok() {
            true => unsafe { buf.as_ptr().offset_from(buffer.as_ptr()) as usize },
            false => 0,
        };

        if matches!(result, Err(Error::PartialRequest(_))) {
            return Err(std::io::Error::from(std::io::ErrorKind::WouldBlock));
        }

        Ok(ParseOk::new(ParseData(result), consumed))
    }
}

impl logger::Klog for ParseData {
    type Response = crate::Response;

    fn klog(&self, _: &Self::Response) {
        // todo: ignore for now
    }
}
