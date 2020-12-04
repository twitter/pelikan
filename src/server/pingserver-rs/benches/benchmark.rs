// Copyright 2020 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use pelikan_pingserver_rs::PingserverBuilder;

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

fn pingserver_benchmark(c: &mut Criterion) {
    // launch the pingserver and wait until its ready
    let pingserver = PingserverBuilder::new(None).spawn();
    std::thread::sleep(Duration::from_millis(100));

    // connect and initialize an empty buffer
    let mut stream = TcpStream::connect("127.0.0.1:12321").expect("failed to connect");
    let mut buffer = vec![0; 4096];

    // define a benchmarking group and duration
    let mut group = c.benchmark_group("request");
    group.measurement_time(Duration::from_secs(60));
    group.throughput(Throughput::Elements(1));

    // benchmark the ping request
    group.bench_function("ping", |b| {
        b.iter(|| {
            assert!(stream.write(b"PING\r\n").is_ok());
            if let Ok(bytes) = stream.read(&mut buffer) {
                if bytes != 6 {
                    panic!("invalid response");
                }
            } else {
                panic!("read error");
            }
        })
    });

    // shutdown the pingserver
    pingserver.shutdown();
}

criterion_group!(benches, pingserver_benchmark);
criterion_main!(benches);
