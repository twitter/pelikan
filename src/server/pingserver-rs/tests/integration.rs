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
    std::thread::sleep(Duration::from_secs(10));

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

    // shutdown server and join
    debug!("shutdown");
    let _ = pingserver.shutdown();
}

fn data(name: &str, data: &[(&str, Option<&str>)]) {
    test("127.0.0.1:12321", name, data)
}

fn admin(name: &str, data: &[(&str, Option<&str>)]) {
    test("127.0.0.1:9999", name, data)
}

// opens a new connection, operating on request + response pairs from the
// provided data.
fn test(endpoint: &str, name: &str, data: &[(&str, Option<&str>)]) {
    info!("testing: {}", name);
    debug!("connecting to server");
    let mut stream = TcpStream::connect(endpoint).expect("failed to connect");
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
