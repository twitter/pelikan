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
use std::str::FromStr;
use std::time::{Duration, SystemTime};

use ccommon::buf::OwnedBuf;
use pelikan_sys::storage::slab::*;
use rustcore::util::{read_buf, write_buf, BufIOError};
use rustcore::worker::WorkerMetrics;

use arrayvec::ArrayVec;
use bytes::{buf::BufMutExt, Buf, BufMut};
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

enum HttpError<'b> {
    Malformed(&'b str),
    Duplicate(&'b str),
    InvalidURI,
    Missing(&'b str),
}

impl<'b> HttpError<'b> {
    fn write_strs<B: BufMut>(
        mut builder: HttpBuilder<B>,
        strs: &[&str],
    ) -> Result<(), RspSendError> {
        let len = strs.iter().map(|s| s.len()).sum();

        builder.header("Content-Length", len)?;
        let mut buf = builder.finish()?;

        if buf.remaining_mut() < len {
            return Err(RspSendError::Compose(httpencode::Error::OutOfBuffer));
        }

        for s in strs {
            buf.put_slice(s.as_bytes());
        }

        Ok(())
    }

    pub fn write_rsp(&self, wbuf: &mut OwnedBuf) -> Result<(), RspSendError> {
        let mut builder = HttpBuilder::response_with_reason(
            wbuf,
            Version::Http11,
            Status::BAD_REQUEST,
            "Bad Request",
        )?;

        builder.header("Content-Type", "text/plain; charset=utf-8")?;

        match self {
            HttpError::Malformed(header) => {
                Self::write_strs(builder, &["Malformed ", header, " header"])
            }
            HttpError::Duplicate(header) => {
                Self::write_strs(builder, &["Duplicate ", header, " header"])
            }
            HttpError::Missing(header) => {
                Self::write_strs(builder, &["Missing ", header, " header"])
            }
            HttpError::InvalidURI => Self::write_strs(builder, &["Invalid URI"]),
        }
    }
}

struct HeaderMap<'h, 'b> {
    headers: &'h mut [Header<'b>],
}

impl<'h, 'b> HeaderMap<'h, 'b> {
    pub fn new(headers: &'h mut [Header<'b>]) -> Result<Self, HttpError<'b>> {
        headers.sort_by(|a, b| order_ascii_lowercase(a.name.as_bytes(), b.name.as_bytes()));

        for window in headers.windows(2) {
            let first = window[0];
            let second = window[1];

            if compare_ascii_lowercase(first.name.as_bytes(), second.name.as_bytes()) {
                return Err(HttpError::Duplicate(first.name));
            }
        }

        Ok(Self { headers })
    }

    pub fn get(&self, name: &str) -> Option<&'b [u8]> {
        let index = self
            .headers
            .binary_search_by(|a| order_ascii_lowercase(a.name.as_bytes(), name.as_bytes()))
            .ok()?;

        Some(unsafe { self.headers.get_unchecked(index).value })
    }

    pub fn get_as<'n, P: FromStr>(&self, name: &'n str) -> Result<Option<P>, HttpError<'n>> {
        let val = match self.get(name) {
            Some(val) => val,
            None => return Ok(None),
        };

        let s = match std::str::from_utf8(val) {
            Ok(s) => s,
            Err(_) => return Err(HttpError::Malformed(name)),
        };

        match s.parse() {
            Ok(v) => Ok(Some(v)),
            Err(_) => Err(HttpError::Malformed(name)),
        }
    }
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
    req: &mut Request<'b, 'h>,
    wbuf: &mut OwnedBuf,
    bytes: &[u8],
) -> Result<Option<usize>, RspSendError> {
    let version = match req.version {
        Some(0) => Version::Http10,
        Some(1) => Version::Http11,
        _ => return unsupported_protocol_version(wbuf).map(|_| None),
    };

    let headers = match HeaderMap::new(req.headers) {
        Ok(info) => info,
        Err(e) => {
            e.write_rsp(wbuf)?;
            return Err(RspSendError::Corrupted);
        }
    };

    let method = req.method.unwrap();
    let path = req.path.unwrap().as_bytes();
    let content_length = match headers.get_as("Content-Length") {
        Ok(bytes) => bytes,
        Err(e) => {
            e.write_rsp(wbuf)?;
            return Err(RspSendError::Corrupted);
        }
    };
    let bytes_read = content_length.unwrap_or(0);

    if bytes_read > bytes.len() {
        return Err(RspSendError::Partial);
    }

    let body = &bytes[..bytes_read];

    let res = match method {
        "GET" => process_get(worker, path, version, wbuf),
        "PUT" => {
            if headers.get("Content-Length").is_none() {
                return HttpError::Missing("Content-Length")
                    .write_rsp(wbuf)
                    .map(|_| None);
            }

            process_put(worker, path, version, body, &headers, wbuf)
        }
        "DELETE" => process_delete(worker, path, version, wbuf),
        _ => invalid_method(wbuf, version),
    };

    res.map(|_| content_length)
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

    builder.header("Content-Type", "application/octet-stream")?;
    builder.header("Content-Length", item.vlen())?;
    builder.header("Flags", flag)?;
    builder.body(&mut value)?;

    Ok(())
}

fn process_put(
    worker: &mut Worker,
    path: &[u8],
    version: Version,
    body: &[u8],
    headers: &HeaderMap,
    wbuf: &mut OwnedBuf,
) -> Result<(), RspSendError> {
    if path[0] != b'/' {
        return HttpError::InvalidURI.write_rsp(wbuf);
    }

    let key = &path[1..];
    let val = body;

    let expiry = match headers.get_as::<HttpDate>("Expiry") {
        Ok(expiry) => match expiry {
            Some(date) => date.into(),
            None => SystemTime::now() + Duration::from_secs(3600 * 24 * 30),
        },
        Err(e) => return e.write_rsp(wbuf),
    };
    let flags = match headers.get_as::<u32>("Flags") {
        Ok(flags) => flags.unwrap_or(0),
        Err(e) => return e.write_rsp(wbuf),
    };

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

async fn handle_malformed<'a, S>(
    error: httparse::Error,
    stream: &'a mut S,
    wbuf: &'a mut OwnedBuf,
    metrics: &'static WorkerMetrics,
) -> Result<(), RspSendError>
where
    S: AsyncRead + AsyncWrite + Unpin + 'static,
{
    metrics.request_parse_ex.incr();

    info!("Got malformed HTTP request: {}", error);

    let mut builder = HttpBuilder::response(wbuf, Version::Http11, Status::BAD_REQUEST)?;
    builder.header("Content-Type", "text/html; charset=utf-8")?;
    let wbuf = builder.finish()?;

    if let Err(_) = write!(wbuf.writer(), "{}", error) {
        return Err(httpencode::Error::OutOfBuffer.into());
    }

    write_buf(stream, wbuf, metrics).await?;

    Ok(())
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
                    if let Err(_) = handle_malformed(e, stream, wbuf, metrics).await {
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
                process_request(&mut borrow, &mut req, wbuf, &slice[bytes..])
            };
            bytes += match process_res {
                Ok(Some(bytes)) => bytes,
                Ok(None) => {
                    // This is the easiest way to make sure we close the connection
                    // after sending a reply.
                    req.version = Some(0);
                    0
                }
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

fn compare_ascii_lowercase(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }

    a.iter()
        .copied()
        .map(|x| x.to_ascii_lowercase())
        .zip(b.iter().copied().map(|x| x.to_ascii_lowercase()))
        .all(|(a, b)| a == b)
}

fn order_ascii_lowercase(a: &[u8], b: &[u8]) -> std::cmp::Ordering {
    use std::cmp::Ordering;

    match a.len().cmp(&b.len()) {
        Ordering::Equal => (),
        x => return x,
    }

    for (a, b) in a.iter().zip(b.iter()) {
        let a = a.to_ascii_lowercase();
        let b = b.to_ascii_lowercase();

        match a.cmp(&b) {
            Ordering::Equal => (),
            x => return x,
        }
    }

    Ordering::Equal
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
