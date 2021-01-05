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

use std::env;
use std::fmt;
use std::io::{Read, Result as IOResult, Write};
use std::net::TcpStream;
use std::ops::Index;
use std::panic::{catch_unwind, resume_unwind};
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use httparse::{Header, Response, Status};
use httpencode::{request, Error, Uri};

struct EscapedByteString<'b>(&'b [u8]);

impl fmt::Display for EscapedByteString<'_> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let iter = self
            .0
            .iter()
            .cloned()
            .flat_map(|x| std::ascii::escape_default(x))
            .map(|x| x as char);

        fmt.write_str("\"")?;
        for c in iter {
            fmt.write_fmt(format_args!("{}", c))?;
        }
        fmt.write_str("\"")?;

        Ok(())
    }
}

struct LinearMap<'b, 'h> {
    headers: &'h [Header<'b>],
}

impl<'b> LinearMap<'b, '_> {
    fn get(&self, key: &str) -> Option<&'b [u8]> {
        for header in self.headers {
            if compare_insensitive(header.name.as_bytes(), key.as_bytes()) {
                return Some(header.value);
            }
        }

        None
    }
}

impl<'b> Index<&'_ str> for LinearMap<'b, '_> {
    type Output = &'b [u8];

    fn index(&self, key: &str) -> &&'b [u8] {
        for header in self.headers {
            if compare_insensitive(header.name.as_bytes(), key.as_bytes()) {
                return &header.value;
            }
        }

        panic!("No such header '{}'\nAll headers: {:?}", key, self.headers);
    }
}

fn escape(bytes: &[u8]) -> EscapedByteString {
    EscapedByteString(bytes)
}

fn put(buf: &mut Vec<u8>, flags: u32, path: &[u8], mut body: &[u8]) -> Result<(), Error> {
    let mut builder = request::put(buf, Uri::new(path))?;

    builder.header("Content-Length", body.len())?;
    builder.header("Flags", flags)?;
    builder.body(&mut body)?;

    Ok(())
}

fn get(buf: &mut Vec<u8>, path: &[u8]) -> Result<(), Error> {
    let mut builder = request::get(buf, Uri::new(path))?;

    builder.header("Content-Length", "0")?;
    builder.finish()?;

    Ok(())
}

fn delete(buf: &mut Vec<u8>, path: &[u8]) -> Result<(), Error> {
    let mut builder = request::delete(buf, Uri::new(path))?;

    builder.header("CoNtEnT-LeNgTh", "0")?;
    builder.finish()?;

    Ok(())
}

fn parse_response<'b, 'h>(
    buffer: &mut &'b [u8],
    headers: &'h mut [Header<'b>],
) -> (u16, LinearMap<'b, 'h>, &'b [u8]) {
    let mut response = Response::new(headers);
    let status = response.parse(*buffer).unwrap();
    let bytes = match status {
        Status::Partial => panic!("Got partial response!"),
        Status::Complete(bytes) => bytes,
    };

    *buffer = &(*buffer)[bytes..];
    let map = LinearMap {
        headers: response.headers,
    };

    let body = match map.get("Content-Length") {
        Some(len) => {
            let len = std::str::from_utf8(len)
                .expect("Content-Length header contained invalid UTF-8")
                .parse()
                .expect("Content-Length was not a number");

            &(*buffer)[..len]
        }
        None => *buffer,
    };

    *buffer = &(*buffer)[body.len()..];

    (response.code.unwrap(), map, body)
}

fn basic_http_test() -> IOResult<()> {
    let mut requests = Vec::new();
    let mut buf = Vec::new();

    // Add a new key to the cache
    put(&mut requests, 63, b"/test", b"test").unwrap();
    // Retrieve that key from the cache
    get(&mut requests, b"/test").unwrap();
    // Delete the key from the cache
    delete(&mut requests, b"/test").unwrap();

    // Try to get the key that's definitely not in the cache
    // since we just removed it.
    // Note: Request with no content-length so that the server
    //       closes the connection.
    request::get(&mut requests, Uri::new(b"/test"))
        .unwrap()
        .finish()
        .unwrap();

    let mut stream = TcpStream::connect("localhost:4779")?;
    stream.set_nodelay(true)?;
    // stream.set_nonblocking(true)?;

    stream.write_all(&requests)?;
    stream.read_to_end(&mut buf)?;

    let mut bytes = &buf[..];
    let mut header_array = [httparse::EMPTY_HEADER; 128];

    println!("resp: {}", escape(bytes));

    let (status, _, body) = parse_response(&mut bytes, &mut header_array);
    assert_eq!(status, 200);
    assert_eq!(body, b"");

    let (status, headers, body) = parse_response(&mut bytes, &mut header_array);
    assert_eq!(status, 200);
    assert_eq!(headers["Flags"], b"63");
    assert_eq!(body, b"test");

    let (status, _, body) = parse_response(&mut bytes, &mut header_array);
    assert_eq!(status, 200);
    assert_eq!(body, b"", "{} != {}", escape(body), escape(b""));

    let (status, _, _) = parse_response(&mut bytes, &mut header_array);
    assert_eq!(status, 404);

    Ok(())
}

fn run_tests() {
    println!("Running test basic_http_test");
    if let Err(e) = basic_http_test() {
        panic!("Failed to run basic_http_test: {}", e);
    }
}

pub fn main() {
    let bindir = match env::args_os().skip(1).next() {
        Some(dir) => PathBuf::from(dir),
        None => {
            eprintln!("Usage: test-pingserver-rs <CMAKE_BINARY_DIR>");
            panic!();
        }
    };

    let mut server = Command::new(bindir.join("_bin/pelikan_twemcache_http"))
        .spawn()
        .expect("Failed to start server process");

    // Give the server time to start up
    std::thread::sleep(Duration::from_millis(1000));

    let res = catch_unwind(|| run_tests());

    // Give the server some time to crash if any of the test
    // would cause that to happen.
    std::thread::sleep(Duration::from_millis(100));

    server.kill().expect("Server died unexpectedly");

    match res {
        Err(e) => resume_unwind(e),
        _ => (),
    }
}

fn compare_insensitive(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }

    a.iter()
        .copied()
        .map(|c| c.to_ascii_lowercase())
        .zip(b.iter().copied().map(|c| c.to_ascii_lowercase()))
        .all(|(a, b)| a == b)
}
