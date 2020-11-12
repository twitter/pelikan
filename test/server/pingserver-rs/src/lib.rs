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
use std::panic::{catch_unwind, resume_unwind};
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

struct EscapedByteString<'b>(&'b [u8]);

impl fmt::Display for EscapedByteString<'_> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let s: String = self
            .0
            .iter()
            .cloned()
            .flat_map(|x| std::ascii::escape_default(x))
            .map(|x| x as char)
            .collect();

        s.fmt(fmt)
    }
}

/// Test that a fragmented ping is properly handled.
fn fragmented_ping() -> IOResult<()> {
    let expected_res: &[u8] = b"PONG\r\n";
    let mut buf = [0u8; b"PONG\r\n".len()];

    let mut stream = TcpStream::connect("127.0.0.1:12321")?;
    stream.set_nodelay(true)?;
    stream.set_nonblocking(false)?;

    stream.write_all(b"PING\r\nPI")?;
    stream.read_exact(&mut buf)?;

    assert_eq!(
        buf,
        expected_res,
        "'{}' != '{}'",
        EscapedByteString(&buf),
        EscapedByteString(expected_res)
    );

    buf = unsafe { std::mem::zeroed() };
    stream.write_all(b"NG\r\n")?;
    stream.read_exact(&mut buf)?;

    assert_eq!(
        buf,
        expected_res,
        "'{}' != '{}'",
        EscapedByteString(&buf),
        EscapedByteString(expected_res)
    );

    Ok(())
}

fn multiping() -> IOResult<()> {
    #[allow(non_upper_case_globals)]
    const expected_res: &[u8] = b"PONG\r\nPONG\r\n";
    let mut buf = [0u8; expected_res.len()];

    let mut stream = TcpStream::connect("127.0.0.1:12321")?;
    stream.set_nodelay(true)?;
    stream.set_nonblocking(false)?;

    stream.write_all(b"PING\r\nPING\r\n")?;
    stream.read_exact(&mut buf)?;

    assert_eq!(
        buf,
        expected_res,
        "'{}' != '{}'",
        EscapedByteString(&buf),
        EscapedByteString(expected_res)
    );

    Ok(())
}

fn partial_ping() -> IOResult<()> {
    let mut stream = TcpStream::connect("127.0.0.1:12321")?;
    stream.set_nodelay(true)?;
    stream.set_nonblocking(false)?;

    stream.write_all(b"PI")
}

fn large_ping() -> IOResult<()> {
    let mut stream = TcpStream::connect("127.0.0.1:12321")?;
    stream.set_nodelay(true)?;
    stream.set_nonblocking(false)?;

    let repeats = 2048;
    let msg = "PING\r\n".repeat(repeats);
    stream.write_all(msg.as_bytes())?;

    let mut buf: [u8; 6];
    for _ in 0..repeats {
        buf = unsafe { std::mem::zeroed() };
        stream.read_exact(&mut buf)?;

        assert_eq!(
            b"PONG\r\n",
            &buf,
            "'{}' != '{}'",
            EscapedByteString(&buf),
            EscapedByteString(b"PONG\r\n")
        );
    }

    Ok(())
}

fn admin_crash() -> IOResult<()> {
    let mut stream = TcpStream::connect("127.0.0.1:9999")?;
    stream.set_nodelay(true)?;
    stream.set_nonblocking(false)?;

    // This set of commands exposed a use-after-free bug in the
    // admin handling thread.
    stream.write_all(b"stats\r\n")?;
    stream.write_all(
        b"Bacon ipsum dolor amet tongue rump pork belly, \
        capicola corned beef sausage kielbasa kevin boudin \
        venison. Pig capicola brisket frankfurter. Filet mignon \
        leberkas shank turducken sirloin bacon porchetta \
        hamburger cow pastrami tongue pork belly drumstick. \
        Frankfurter jerky pork chop, landjaeger strip steak \
        meatball kielbasa chicken turkey venison. Bacon drumstick \
        prosciutto, sausage tri-tip buffalo chuck chicken. Filet \
        mignon pastrami prosciutto jerky corned beef boudin pork belly \
        landjaeger short ribs tail jowl chicken drumstick t-bone capicola. \
        Rump spare ribs landjaeger, pork belly jowl kielbasa fatback.\r\n",
    )?;
    stream.write_all(b"quit\r\n")?;

    Ok(())
}

fn run_tests() {
    println!("Running test multiping");
    if let Err(e) = multiping() {
        panic!("test multiping failed: {}", e);
    }

    println!("Running test fragmented_ping");
    if let Err(e) = fragmented_ping() {
        panic!("test fragmented_ping failed: {}", e);
    }

    println!("Running test partial_ping");
    if let Err(e) = partial_ping() {
        panic!("test partial_ping failed: {}", e);
    }

    println!("Running test large_ping");
    if let Err(e) = large_ping() {
        panic!("test large_ping failed: {}", e);
    }

    println!("Running test admin_crash");
    if let Err(e) = admin_crash() {
        panic!("test admin_crash failed: {}", e);
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

    let mut server = Command::new(bindir.join("_bin/pelikan_pingserver_rs"))
        .spawn()
        .expect("Failed to start server process");

    // Give the server time to start up
    std::thread::sleep(Duration::from_millis(500));

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
