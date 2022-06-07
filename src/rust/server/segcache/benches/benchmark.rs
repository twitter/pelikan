// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! This is a very basic benchmark which tests only get requests with a few
//! different key and value sizes. It's only using one connection and a very
//! primitive blocking client, so these results do not reflect the true
//! performance of the server when under load. It can be used to get a rough
//! idea of how changes may impact performance.
//!
//! For formal performance testing, it is recommended to use
//! [rpc-perf](https://github.com/twitter/rpc-perf) or another cache
//! benchmarking tool which supports the Memcache ASCII protocol.

use config::SegcacheConfig;
use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use pelikan_segcache_rs::Segcache;

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

fn get_benchmark(c: &mut Criterion) {
    // use the default config
    let config = SegcacheConfig::default();

    // launch the server
    let server = Segcache::new(config).expect("failed to launch segcache");

    // wait for server to startup. duration is chosen to be longer than we'd
    // expect startup to take in a slow ci environment.
    std::thread::sleep(Duration::from_secs(10));

    // connect and initialize an empty buffer
    let mut stream = TcpStream::connect("127.0.0.1:12321").expect("failed to connect");
    let mut buffer = vec![0; 1024 * 1024];

    // define a benchmarking group
    let mut group = c.benchmark_group("request");
    group.throughput(Throughput::Elements(1));

    let mut key_id = 0;

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
                    assert_eq!(&buffer[0..bytes], b"END\r\n", "invalid response");
                } else {
                    panic!("read error");
                }
            })
        });

        // benchmark across a few value lengths
        for vlen in [1, 64, 1024, 4096].iter() {
            let key = format!("{:01$}", key_id, klen);
            let value = format!("{:A>1$}", 0, vlen);
            let msg = format!("set {} 0 0 {}\r\n{}\r\n", key, vlen, value);
            assert!(stream.write_all(msg.as_bytes()).is_ok());
            if let Ok(bytes) = stream.read(&mut buffer) {
                assert_eq!(&buffer[0..bytes], b"STORED\r\n", "invalid response");
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
                        assert_eq!(&buffer[0..bytes], response.as_bytes(), "invalid response");
                    } else {
                        panic!("read error");
                    }
                })
            });

            key_id += 1;
        }
    }

    // shutdown the server
    server.shutdown();
}

criterion_group!(benches, get_benchmark);
criterion_main!(benches);
