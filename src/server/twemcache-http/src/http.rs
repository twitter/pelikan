// Copyright (C) 2019 Twitter, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::cell::RefCell;
use std::io::Write;
use std::rc::Rc;
use std::time::{Duration, SystemTime};

use ccommon::buf::OwnedBuf;
use pelikan_sys::storage::slab::*;
use rustcore::util::{read_buf, write_buf, BufIOError};
use rustcore::worker::WorkerMetrics;

use arrayvec::ArrayVec;
use bytes::{buf::BufMutExt, Buf};
use httparse::{Header, Request, Status as ParseStatus, EMPTY_HEADER};
use httpdate::HttpDate;
use httpencode::{HttpBuilder, Status, Version};
use tokio::io::{AsyncRead, AsyncWrite};

use crate::worker::Worker;

#[derive(Error, Debug)]
enum RspSendError {
    #[error("{0}")]
    Compose(#[from] httpencode::Error),
    #[error("{0}")]
    BufIO(#[from] BufIOError),
    #[error("Partial response")]
    Partial,
    #[error("Corrupted request")]
    // Got enough of a request to send a response but
    // can't guarantee anything about the data that
    // follows. Send an error then close the connection.
    Corrupted,
}

enum HttpError {
    MalformedContentLength,
    DuplicateContentLength,
    MissingContentLength,
    InvalidURI,
    MalformedExpires,
    DuplicateExpires,
    MalformedFlag,
    DuplicateFlag,
}

impl HttpError {
    pub fn write_rsp(&self, wbuf: &mut OwnedBuf) -> Result<(), RspSendError> {
        let message = match self {
            HttpError::MalformedContentLength => "Malformed Content-Length header",
            HttpError::DuplicateContentLength => "Duplicate Content-Length header",
            HttpError::MissingContentLength => "Missing Content-Length header",
            HttpError::InvalidURI => "Invalid URI",
            HttpError::MalformedExpires => "Malformed Expires header",
            HttpError::DuplicateExpires => "Duplicate Expires header",
            HttpError::MalformedFlag => "Malformed Flag header",
            HttpError::DuplicateFlag => "Duplicate Flag header",
        };

        let mut builder = HttpBuilder::response_with_reason(
            wbuf,
            Version::Http11,
            Status::BAD_REQUEST,
            "Bad Request",
        )?;

        builder.header("Content-Type", "text/plain; charset=utf-8")?;
        builder.header("Content-Length", message.len())?;
        builder.body(&mut message.as_bytes())?;

        Ok(())
    }
}

#[derive(Default, Debug)]
struct HeaderInfo {
    pub content_length: Option<usize>,
    pub expiry: Option<SystemTime>,
    pub flag: Option<u32>,
}

impl HeaderInfo {
    pub fn from_headers(headers: &[Header]) -> Result<Self, HttpError> {
        const U32_MAX: usize = std::u32::MAX as usize;

        let mut info = Self::default();

        for header in headers {
            let name = header.name.as_bytes();

            if compare_ascii_lowercase(name, b"content-length") {
                if info.content_length.is_some() {
                    return Err(HttpError::DuplicateContentLength);
                }

                info.content_length = match parse_usize(header.value) {
                    Some(x) => Some(x),
                    None => return Err(HttpError::MalformedContentLength),
                };
            } else if compare_ascii_lowercase(name, b"expires") {
                if info.expiry.is_some() {
                    return Err(HttpError::DuplicateExpires);
                }

                let date: HttpDate = std::str::from_utf8(header.value)
                    .map_err(|_| HttpError::MalformedExpires)?
                    .parse()
                    .map_err(|_| HttpError::MalformedExpires)?;

                info.expiry = Some(date.into());
            } else if compare_ascii_lowercase(name, b"flag") {
                if info.expiry.is_some() {
                    return Err(HttpError::DuplicateFlag);
                }

                info.flag = match parse_usize(header.value) {
                    val @ Some(0..=U32_MAX) => val.map(|x| x as u32),
                    _ => return Err(HttpError::MalformedFlag),
                };
            }
        }

        Ok(info)
    }
}

async fn malformed_request<'a, S>(
    stream: &'a mut S,
    mut builder: HttpBuilder<&'a mut OwnedBuf>,
    error: httparse::Error,
    metrics: &'static WorkerMetrics,
) -> Result<(), RspSendError>
where
    S: AsyncWrite + Unpin + 'static,
{
    builder.header("Content-Type", "text/html; charset=utf-8")?;

    let buf = builder.finish()?;

    if let Err(_) = write!(buf.writer(), "{}", error) {
        return Err(httpencode::Error::OutOfBuffer.into());
    }

    write_buf(stream, buf, metrics).await?;

    Ok(())
}

fn key_not_found(wbuf: &mut OwnedBuf, version: Version) -> Result<(), RspSendError> {
    let mut builder = HttpBuilder::response(wbuf, version, Status::NOT_FOUND)?;

    builder.header("Content-Length", "0")?;
    builder.finish()?;

    Ok(())
}

fn unsupported_protocol_version(wbuf: &mut OwnedBuf) -> Result<(), RspSendError> {
    let mut builder =
        HttpBuilder::response(wbuf, Version::Http10, Status::HTTP_VERSION_NOT_SUPPORTED)?;

    builder.header("Content-Length", 0)?;
    builder.finish()?;

    Ok(())
}

fn invalid_method(wbuf: &mut OwnedBuf, version: Version) -> Result<(), RspSendError> {
    let mut builder = HttpBuilder::response(wbuf, version, Status::METHOD_NOT_ALLOWED)?;

    let message = "This server only supports GET, PUT, and DELETE";

    builder.header("Content-Length", 0)?;
    builder.header("Content-Type", "text/plain")?;
    builder.header("Content-Length", message.len())?;
    builder.header("Allow", "GET, PUT, DELETE")?;
    builder.body(&mut message.as_bytes())?;

    Ok(())
}

fn process_request<'b, 'h>(
    worker: &mut Worker,
    req: &Request<'b, 'h>,
    wbuf: &mut OwnedBuf,
    bytes: &[u8],
) -> Result<usize, RspSendError> {
    let version = match req.version {
        Some(0) => Version::Http10,
        Some(1) => Version::Http11,
        _ => return unsupported_protocol_version(wbuf).map(|_| 0),
    };

    let info = match HeaderInfo::from_headers(req.headers) {
        Ok(info) => info,
        Err(e) => {
            e.write_rsp(wbuf)?;
            return Err(RspSendError::Corrupted);
        }
    };

    let method = req.method.unwrap();
    let path = req.path.unwrap().as_bytes();
    let bytes_read = info.content_length.unwrap_or(0);

    info!("headers: {:?}", info);
    info!("body available: {}", escape(bytes));
    info!("path: {}", req.path.unwrap());

    if bytes_read > bytes.len() {
        return Err(RspSendError::Partial);
    }

    let body = &bytes[..bytes_read];

    info!("req body: {}", escape(body));
    info!("bytes read: {}", bytes_read);

    let res = match method {
        "GET" => process_get(worker, path, version, wbuf),
        "PUT" => {
            if info.content_length.is_none() {
                return HttpError::MissingContentLength.write_rsp(wbuf).map(|_| 0);
            }

            process_put(worker, path, version, body, &info, wbuf)
        }
        "DELETE" => process_delete(worker, path, version, wbuf),
        _ => invalid_method(wbuf, version),
    };

    res.map(|_| bytes_read)
}

fn process_get(
    worker: &mut Worker,
    path: &[u8],
    version: Version,
    wbuf: &mut OwnedBuf,
) -> Result<(), RspSendError> {
    if path[0] != b'/' {
        return HttpError::InvalidURI.write_rsp(wbuf);
    }
    let key = &path[1..];

    let item = match worker.get(key) {
        Some(item) => item,
        None => return key_not_found(wbuf, version),
    };

    let mut value =
        unsafe { std::slice::from_raw_parts(item_data(item) as *mut u8, item.vlen() as usize) };

    let mut builder = HttpBuilder::response(wbuf, version, Status::OK)?;

    let flag = unsafe { *(item_optional(item) as *mut u32) };

    info!("got value: {:?}", escape(value));

    builder.header("Content-Type", "application/octet-stream")?;
    builder.header("Content-Length", item.vlen())?;
    builder.header("Flag", flag)?;
    builder.body(&mut value)?;

    Ok(())
}

fn process_put(
    worker: &mut Worker,
    path: &[u8],
    version: Version,
    body: &[u8],
    info: &HeaderInfo,
    wbuf: &mut OwnedBuf,
) -> Result<(), RspSendError> {
    if path[0] != b'/' {
        return HttpError::InvalidURI.write_rsp(wbuf);
    }

    let key = &path[1..];
    let val = body;

    let expiry = info
        .expiry
        .unwrap_or_else(|| SystemTime::now() + Duration::from_secs(3600 * 24 * 30));
    let flags = info.flag.unwrap_or(0);

    if let Err(e) = worker.put(key, val, expiry, flags) {
        // The client submitted a request that made the backing
        // storage unhappy.
        let mut builder = HttpBuilder::response(wbuf, version, Status::BAD_REQUEST)?;
        let mut body = ArrayVec::<[u8; 32]>::new();
        write!(&mut body, "{}", e).expect("ItemError formatted to an item longer than 32 bytes");

        builder.header("Content-Length", body.len())?;
        builder.header("Content-Type", "text/plain")?;
        builder.body(&mut body.as_slice())?;

        return Ok(());
    };

    let mut builder = HttpBuilder::response(wbuf, version, Status::OK)?;
    builder.header("Content-Length", 0)?;
    builder.finish()?;

    Ok(())
}

fn process_delete(
    worker: &mut Worker,
    path: &[u8],
    version: Version,
    wbuf: &mut OwnedBuf,
) -> Result<(), RspSendError> {
    if path[0] != b'/' {
        return HttpError::InvalidURI.write_rsp(wbuf);
    }

    let key = &path[1..];
    if !worker.delete(key) {
        let mut builder = HttpBuilder::response(wbuf, version, Status::NOT_FOUND)?;

        builder.header("Content-Length", "0")?;
        builder.finish()?;

        return Ok(());
    }

    let mut builder = HttpBuilder::response(wbuf, version, Status::OK)?;
    builder.header("Content-Length", "0")?;
    builder.finish()?;

    return Ok(());
}

pub async fn http_worker<'a, S>(
    worker: Rc<RefCell<Worker>>,
    stream: &'a mut S,
    rbuf: &'a mut OwnedBuf,
    wbuf: &'a mut OwnedBuf,
    metrics: &'static WorkerMetrics,
) where
    S: AsyncRead + AsyncWrite + Unpin + 'static,
{
    loop {
        if let Err(e) = read_buf(stream, rbuf, metrics).await {
            match e {
                BufIOError::StreamClosed | BufIOError::IOError(_) => return,
                e => {
                    warn!("Failed to read from stream: {}", e);
                    return;
                }
            }
        }

        let mut headers = [EMPTY_HEADER; 64];
        let mut slice = rbuf.as_slice();
        let mut advanced = 0;

        while slice.len() > 0 {
            let mut req = Request::new(&mut headers);
            let status = match req.parse(slice) {
                Ok(status) => status,
                Err(e) => {
                    metrics.request_parse_ex.incr();

                    info!("Got malformed HTTP request: {}", e);

                    let builder =
                        match HttpBuilder::response(wbuf, Version::Http11, Status::BAD_REQUEST) {
                            Ok(builder) => builder,
                            Err(_) => {
                                metrics.response_compose_ex.incr();
                                warn!("Failed to write response");
                                return;
                            }
                        };

                    if let Err(_) = malformed_request(stream, builder, e, metrics).await {
                        metrics.response_compose_ex.incr();
                    }

                    return;
                }
            };

            let mut bytes = match status {
                ParseStatus::Complete(nbytes) => nbytes,
                ParseStatus::Partial => break,
            };

            info!("Got HTTP request: {:?}", req);

            let process_res = {
                let mut borrow = worker.borrow_mut();
                process_request(&mut borrow, &req, wbuf, &slice[bytes..])
            };
            bytes += match process_res {
                Ok(bytes) => bytes,
                Err(e) => match e {
                    RspSendError::Partial => break,
                    RspSendError::Corrupted => {
                        let _ = write_buf(stream, wbuf, metrics).await;
                        warn!("Got corrupted HTTP header");
                        return;
                    }
                    _ => return,
                },
            };

            advanced += bytes;
            slice = &slice[bytes..];

            if let Err(e) = write_buf(stream, wbuf, metrics).await {
                warn!("Failed to send to connection: {}", e);
                return;
            }

            // HTTP/1.0 doesn't support multiple requests in the same
            // stream. If clients want to take advantage of that they
            // need to use HTTP/1.1
            if req.version != Some(1) {
                return;
            }
        }

        rbuf.advance(advanced);
        rbuf.lshift();
    }
}

pub fn parse_usize(bytes: &[u8]) -> Option<usize> {
    fn trim(mut slice: &[u8]) -> &[u8] {
        while slice.first().map(u8::is_ascii_whitespace).unwrap_or(false) {
            slice = slice.split_first().map(|x| x.1).unwrap_or(slice);
        }

        while slice.last().map(u8::is_ascii_whitespace).unwrap_or(false) {
            slice = slice.split_last().map(|x| x.1).unwrap_or(slice);
        }

        slice
    }

    let bytes = trim(bytes);
    let mut value: usize = 0;

    for digit in bytes.iter().copied() {
        let digit: u8 = digit;
        if !digit.is_ascii_digit() {
            return None;
        }

        value = value
            .checked_mul(10)?
            .checked_add((digit - b'0') as usize)?;
    }

    Some(value)
}

/// Note: assumes that `b` is already in lowercase
fn compare_ascii_lowercase(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }

    a.iter()
        .copied()
        .map(|x| x.to_ascii_lowercase())
        .zip(b.iter().copied())
        .all(|(a, b)| a == b)
}

fn escape(bytes: &[u8]) -> String {
    bytes
        .iter()
        .copied()
        .flat_map(std::ascii::escape_default)
        .map(|c| c as char)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_case_insensitive_compare() {
        assert!(compare_ascii_lowercase(b"AAAAA", b"aaaaa"));
        assert!(compare_ascii_lowercase(
            b"Content-Length",
            b"content-length"
        ));
    }
}
