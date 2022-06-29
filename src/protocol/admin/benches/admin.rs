// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Basic benchmarks to test performance of the protocol implementation.

use core::time::Duration;
use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use protocol_admin::*;

const KB: usize = 1024;
const BUFFER_SIZE: usize = 16 * KB;
const DURATION: u64 = 30; // seconds

fn version(c: &mut Criterion) {
    let parser = AdminRequestParser::new();

    let mut group = c.benchmark_group("version");
    group.measurement_time(Duration::from_secs(DURATION));
    group.throughput(Throughput::Elements(1));

    let mut buffer = Vec::new();
    buffer.extend_from_slice(b"version\r\n");
    buffer.resize_with(BUFFER_SIZE, Default::default);

    group.bench_function("parse", |b| {
        b.iter(|| {
            let _ = parser.parse(&buffer);
        })
    });
}

criterion_group!(benches, version);
criterion_main!(benches);
