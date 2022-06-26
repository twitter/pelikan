// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Basic benchmarks to test performance of the protocol implementation.

use core::time::Duration;
use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use protocol_common::Parse;

use protocol_resp::*;

const KB: usize = 1024;

const BUFFER_SIZE: usize = 16 * KB;
const DURATION: u64 = 30; // seconds

fn get(c: &mut Criterion) {
    let parser = RequestParser::new();

    let mut group = c.benchmark_group("get");
    group.measurement_time(Duration::from_secs(DURATION));
    group.throughput(Throughput::Elements(1));

    let mut buffer = Vec::new();
    buffer.extend_from_slice(b"get 0\r\n");
    buffer.resize_with(BUFFER_SIZE, Default::default);

    group.bench_function("1b", |b| {
        b.iter(|| {
            let _ = parser.parse(&buffer);
        })
    });
}

fn set(c: &mut Criterion) {
    let parser = RequestParser::new();

    let mut group = c.benchmark_group("set");
    group.measurement_time(Duration::from_secs(DURATION));
    group.throughput(Throughput::Elements(1));

    let mut buffer = Vec::new();
    buffer.extend_from_slice(b"set 0 0\r\n");
    buffer.resize_with(BUFFER_SIZE, Default::default);

    group.bench_function("1b/1b", |b| {
        b.iter(|| {
            let _ = parser.parse(&buffer);
        })
    });

    let mut buffer = Vec::new();
    buffer.extend_from_slice(b"set 0 0123456789\r\n");
    buffer.resize_with(BUFFER_SIZE, Default::default);

    group.bench_function("1b/10b", |b| {
        b.iter(|| {
            let _ = parser.parse(&buffer);
        })
    });

    let mut buffer = Vec::new();
    buffer.extend_from_slice(b"set 0 0123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789\r\n");
    buffer.resize_with(BUFFER_SIZE, Default::default);

    group.bench_function("1b/100b", |b| {
        b.iter(|| {
            let _ = parser.parse(&buffer);
        })
    });
}

criterion_group!(benches, get, set);
criterion_main!(benches);
