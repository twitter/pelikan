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

    let mut stream = TcpStream::connect("localhost:12321")?;
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

    let mut stream = TcpStream::connect("localhost:12321")?;
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
    let mut stream = TcpStream::connect("localhost:12321")?;
    stream.set_nodelay(true)?;
    stream.set_nonblocking(false)?;

    stream.write_all(b"PI")
}

fn run_tests() {
    if let Err(e) = multiping() {
        panic!("test multiping failed: {}", e);
    }

    if let Err(e) = fragmented_ping() {
        panic!("test fragmented_ping failed: {}", e);
    }

    if let Err(e) = partial_ping() {
        panic!("test partial_ping failed: {}", e);
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

    server.kill().expect("Server died unexpectedly");

    match res {
        Err(e) => resume_unwind(e),
        _ => (),
    }
}
