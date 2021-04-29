// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

#[macro_use]
extern crate rustcommon_logger;

use pelikan_twemcache_rs::TwemcacheBuilder;

use rustcommon_logger::{Level, Logger};

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

fn main() {
    // initialize logging
    Logger::new()
        .label("test")
        .level(Level::Info)
        .init()
        .expect("Failed to initialize logger");

    debug!("launching server");
    let server = TwemcacheBuilder::new(None).spawn();

    // wait for server to startup. duration is chosen to be longer than we'd
    // expect startup to take in a slow ci environment.
    std::thread::sleep(Duration::from_secs(10));

    debug!("beginning tests");
    println!();

    test("get empty (key: 0)", &[("get 0\r\n", Some("END\r\n"))]);
    test("gets empty (key: 0)", &[("gets 0\r\n", Some("END\r\n"))]);
    test(
        "cas not found (key: 0)",
        &[("cas 0 0 0 1 0\r\n0\r\n", Some("NOT_FOUND\r\n"))],
    );
    test(
        "set value (key: 0)",
        &[("set 0 0 0 1\r\n1\r\n", Some("STORED\r\n"))],
    );
    test(
        "get value (key: 0)",
        &[("get 0\r\n", Some("VALUE 0 0 1\r\n1\r\nEND\r\n"))],
    );
    test(
        "gets value (key: 0)",
        &[("gets 0\r\n", Some("VALUE 0 0 1 1\r\n1\r\nEND\r\n"))],
    );
    test(
        "cas fail (key: 0)",
        &[("cas 0 0 0 1 0\r\n1\r\n", Some("EXISTS\r\n"))],
    );
    test(
        "cas success (key: 0)",
        &[("cas 0 0 0 1 1\r\n1\r\n", Some("STORED\r\n"))],
    );
    test(
        "add value (key: 0)",
        &[("add 0 0 0 1\r\n2\r\n", Some("NOT_STORED\r\n"))],
    );
    test(
        "add value (key: 1)",
        &[("add 1 0 0 1\r\n2\r\n", Some("STORED\r\n"))],
    );
    test(
        "get value (key: 0)",
        &[("get 0\r\n", Some("VALUE 0 0 1\r\n1\r\nEND\r\n"))],
    );
    test(
        "get value (key: 1)",
        &[("get 1\r\n", Some("VALUE 1 0 1\r\n2\r\nEND\r\n"))],
    );
    test(
        "replace value (key: 0)",
        &[("replace 0 0 0 1\r\n2\r\n", Some("STORED\r\n"))],
    );
    test(
        "replace value (key: 2)",
        &[("replace 2 0 0 1\r\n2\r\n", Some("NOT_STORED\r\n"))],
    );
    test(
        "get value (key: 0)",
        &[("get 0\r\n", Some("VALUE 0 0 1\r\n2\r\nEND\r\n"))],
    );
    test("get value (key: 2)", &[("get 2\r\n", Some("END\r\n"))]);

    // test storing and retrieving flags
    test(
        "set value (key: 3)",
        &[("set 3 42 0 1\r\n1\r\n", Some("STORED\r\n"))],
    );
    test(
        "get value (key: 3)",
        &[("get 3\r\n", Some("VALUE 3 42 1\r\n1\r\nEND\r\n"))],
    );

    test("quit", &[("quit\r\n", Some(""))]);

    // shutdown server and join
    debug!("shutdown");
    let _ = server.shutdown();
}

// opens a new connection, operating on request + response pairs from the
// provided data.
fn test(name: &str, data: &[(&str, Option<&str>)]) {
    info!("testing: {}", name);
    debug!("connecting to server");
    let mut stream = TcpStream::connect("127.0.0.1:12321").expect("failed to connect");
    stream
        .set_read_timeout(Some(Duration::from_millis(250)))
        .expect("failed to set read timeout");
    stream
        .set_write_timeout(Some(Duration::from_millis(250)))
        .expect("failed to set write timeout");

    debug!("sending request");
    for (request, response) in data {
        match stream.write(request.as_bytes()) {
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

        std::thread::sleep(Duration::from_millis(10));
        let mut buf = vec![0; 4096];

        if let Some(response) = response {
            if stream.read(&mut buf).is_err() {
                fatal!("error reading response");
            } else {
                if response.as_bytes() != &buf[0..response.len()] {
                    error!("expected: {:?}", response.as_bytes());
                    error!("received: {:?}", &buf[0..response.len()]);
                    fatal!("status: failed\n");
                } else {
                    debug!("correct response");
                }
            }
            assert_eq!(response.as_bytes(), &buf[0..response.len()]);
        } else {
            if let Err(e) = stream.read(&mut buf) {
                if e.kind() == std::io::ErrorKind::WouldBlock {
                    debug!("got no response");
                } else {
                    error!("error reading response");
                    fatal!("status: failed\n");
                }
            } else {
                error!("expected no response");
                fatal!("status: failed\n");
            }
        }

        if data.len() > 1 {
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    info!("status: passed\n");
}
