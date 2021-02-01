// Copyright 2020 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

#[macro_use]
extern crate rustcommon_logger;

use pelikan_pingserver_rs::PingserverBuilder;

use rustcommon_logger::{Level, Logger};

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

pub const ADMIN_ENDPOINT: &str = "127.0.0.1:9999";
pub const SERVER_ENDPOINT: &str = "127.0.0.1:12321";

pub const STARTUP_DELAY: Duration = Duration::from_secs(10);
pub const TIMEOUT: Duration = Duration::from_millis(250);

fn main() {
    // initialize logging
    Logger::new()
        .label("test")
        .level(Level::Info)
        .init()
        .expect("Failed to initialize logger");

    debug!("launching server");
    let pingserver = PingserverBuilder::new(None).spawn();

    // wait for server to startup. duration is chosen to be longer than we'd
    // expect startup to take in a slow ci environment.
    std::thread::sleep(STARTUP_DELAY);

    debug!("beginning tests");
    println!();

    data("ping", &[("PING\r\n", Some("PONG\r\n"))]);
    data(
        "multiping",
        &[("PING\r\nPING\r\n", Some("PONG\r\nPONG\r\n"))],
    );
    data("partial", &[("PI", None)]);
    data("fragmented", &[("PI", None), ("NG\r\n", Some("PONG\r\n"))]);
    data("quit", &[("QUIT\r\n", Some(""))]);

    admin("admin invalid", &[("INVALID REQUEST\r\n", Some(""))]);
    admin_stats();

    // shutdown server and join
    debug!("shutdown");
    let _ = pingserver.shutdown();
}

fn data(name: &str, data: &[(&str, Option<&str>)]) {
    test(SERVER_ENDPOINT, name, data)
}

fn admin(name: &str, data: &[(&str, Option<&str>)]) {
    test(ADMIN_ENDPOINT, name, data)
}

fn connect(endpoint: &str) -> TcpStream {
    debug!("connecting to server");
    let stream = TcpStream::connect(endpoint).expect("failed to connect");
    stream
        .set_read_timeout(Some(TIMEOUT))
        .expect("failed to set read timeout");
    stream
        .set_write_timeout(Some(TIMEOUT))
        .expect("failed to set write timeout");
    stream
}

fn send_request(stream: &mut TcpStream, request: &[u8]) {
    debug!("sending request");
    match stream.write(request) {
        Ok(bytes) => {
            if bytes == request.len() {
                debug!("full request sent");
            } else {
                error!("incomplete write");
                fatal!("status: failed\n");
            }
        }
        Err(_) => {
            error!("error sending request");
            fatal!("status: failed\n");
        }
    }
}

fn get_response(stream: &mut TcpStream, buf: &mut [u8]) -> usize {
    match stream.read(buf) {
        Ok(0) => {
            error!("server hangup");
            fatal!("status: failed\n");
        }
        Err(e) => {
            if e.kind() == std::io::ErrorKind::WouldBlock {
                debug!("got no response");
                0
            } else {
                error!("error reading response");
                fatal!("status: failed\n");
            }
        }
        Ok(n) => n,
    }
}

fn did_hangup(stream: &mut TcpStream, buf: &mut [u8]) -> bool {
    match stream.read(buf) {
        Ok(0) => {
            true
        }
        Err(e) => {
            if e.kind() == std::io::ErrorKind::WouldBlock {
                debug!("got no response");
                false
            } else {
                error!("error reading response");
                fatal!("status: failed\n");
            }
        }
        Ok(_) => false,
    }
}

fn admin_stats() {
    info!("testing: admin stats");
    let mut stream = connect(ADMIN_ENDPOINT);
    send_request(&mut stream, b"stats\r\n");
    std::thread::sleep(Duration::from_millis(10));
    let mut buf = vec![0; 4096];
    let bytes = get_response(&mut stream, &mut buf);
    if buf.len() < 7 {
        error!("response too short");
        fatal!("status: failed\n");
    } else if buf[bytes - 5..bytes] != *b"END\r\n" {
        error!("incorrectly terminated response");
        fatal!("status: failed\n");
    } else {
        debug!("correctly terminated response");
        info!("status: passed\n")
    }
}

// opens a new connection, operating on request + response pairs from the
// provided data.
fn test(endpoint: &str, name: &str, data: &[(&str, Option<&str>)]) {
    info!("testing: {}", name);
    debug!("connecting to server");
    let mut stream = connect(endpoint);

    for (request, response) in data {
        send_request(&mut stream, request.as_bytes());

        std::thread::sleep(Duration::from_millis(10));
        let mut buf = vec![0; 4096];

        if let Some(response) = response {
            if response.is_empty() {
                if !did_hangup(&mut stream, &mut buf) {
                    error!("server didn't hangup");
                    fatal!("status: failed");
                }
            } else {
                let _ = get_response(&mut stream, &mut buf);
                if response.as_bytes() != &buf[0..response.len()] {
                    error!("expected: {:?}", response.as_bytes());
                    error!("received: {:?}", &buf[0..response.len()]);
                    fatal!("status: failed\n");
                } else {
                    debug!("correct response");
                }
            }
        } else if get_response(&mut stream, &mut buf) != 0 {
            error!("expected nothing but got a response");
            fatal!("status: failed\n");
        }
        if data.len() > 1 {
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    info!("status: passed\n");
}
