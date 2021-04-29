// Copyright 2020 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

// This is a very basic benchmark which tests only get requests with a few
// different key and value sizes. It's only using one connection and a very
// primitive blocking client, so these results do not reflect the true
// performance of the server when under load. It can be used to get a rough idea
// of how changes may impact performance.

// For formal performance testing, it is recommended to use
// https://github.com/twitter/rpc-perf or another memcached benchmarking tool.

use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use pelikan_twemcache_rs::TwemcacheBuilder;

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

fn get_benchmark(c: &mut Criterion) {
    // launch the server
    let server = TwemcacheBuilder::new(None).spawn();

    // wait for server to startup. duration is chosen to be longer than we'd
    // expect startup to take in a slow ci environment.
    std::thread::sleep(Duration::from_secs(10));

    // connect and initialize an empty buffer
    let mut stream = TcpStream::connect("127.0.0.1:12321").expect("failed to connect");
    let mut buffer = vec![0; 1024 * 1024];

    // define a benchmarking group
    let mut group = c.benchmark_group("request");
    group.throughput(Throughput::Elements(1));

    // benchmark for a few key lengths
    for klen in [1, 16, 64, 255].iter() {
        // benchmark getting empty value
        let bench_name = format!("get/{}b/0b", klen);
        let key = format!("{:01$}", 0, klen);
        let msg = format!("get {}\r\n", key);
        group.bench_function(&bench_name, |b| {
            b.iter(|| {
                assert!(stream.write_all(msg.as_bytes()).is_ok());
                if let Ok(bytes) = stream.read(&mut buffer) {
                    if &buffer[0..bytes] != b"END\r\n" {
                        panic!("invalid response");
                    }
                } else {
                    panic!("read error");
                }
            })
        });

        // benchmark across a few value lengths
        for (key, vlen) in [1, 64, 1024, 4096].iter().enumerate() {
            let key = format!("{:01$}", key, klen);
            let value = format!("{:01$}", 0, vlen);
            let msg = format!("set {} 0 0 {}\r\n{}\r\n", key, vlen, value);
            assert!(stream.write_all(msg.as_bytes()).is_ok());
            if let Ok(bytes) = stream.read(&mut buffer) {
                if &buffer[0..bytes] != b"STORED\r\n" {
                    panic!("invalid response: {:?}", &buffer[0..bytes]);
                }
            } else {
                panic!("read error");
            }

            let bench_name = format!("get/{}b/{}b", klen, vlen);
            let msg = format!("get {}\r\n", key);
            let response = format!("VALUE {} 0 {}\r\n{}\r\nEND\r\n", key, vlen, value);
            group.bench_function(&bench_name, |b| {
                b.iter(|| {
                    assert!(stream.write_all(msg.as_bytes()).is_ok());
                    if let Ok(bytes) = stream.read(&mut buffer) {
                        if &buffer[0..bytes] != response.as_bytes() {
                            panic!("invalid response");
                        }
                    } else {
                        panic!("read error");
                    }
                })
            });
        }
    }

    // shutdown the server
    server.shutdown();
}

criterion_group!(benches, get_benchmark);
criterion_main!(benches);
