// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! This is a very basic benchmark which tests the performance of the server.
//! It's only using one connection and a very primitive blocking client, so
//! these results do not reflect the true performance of the server when under
//! load. It can be used to get a rough idea of how changes may impact
//! performance.
//!
//! For formal performance testing, it is recommended to use
//! [rpc-perf](https://github.com/twitter/rpc-perf).

use config::PingserverConfig;
use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use pelikan_pingserver_rs::Pingserver;

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

fn ping_benchmark(c: &mut Criterion) {
    let config = PingserverConfig::default();

    // launch the server
    let server = Pingserver::new(config).expect("failed to launch pingserver");

    // wait for server to startup. duration is chosen to be longer than we'd
    // expect startup to take in a slow ci environment.
    std::thread::sleep(Duration::from_secs(10));

    // connect and initialize an empty buffer
    let mut stream = TcpStream::connect("127.0.0.1:12321").expect("failed to connect");
    let mut buffer = vec![0; 1024 * 1024];

    // define a benchmarking group
    let mut group = c.benchmark_group("request");
    group.throughput(Throughput::Elements(1));

    let msg = b"ping\r\n";
    let bench_name = "ping";

    group.bench_function(bench_name, |b| {
        b.iter(|| {
            assert!(stream.write_all(msg).is_ok());
            if let Ok(bytes) = stream.read(&mut buffer) {
                if &buffer[0..bytes] != b"PONG\r\n" {
                    panic!("invalid response");
                }
            } else {
                panic!("read error");
            }
        })
    });

    // shutdown the server
    server.shutdown();
}

criterion_group!(benches, ping_benchmark);
criterion_main!(benches);
