// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use bloom::BloomFilter;
use criterion::{black_box, criterion_group, criterion_main, Criterion};

const MB: usize = 1024 * 1024;

fn small_key(c: &mut Criterion) {
    c.bench_function("unit", |b| {
        let mut bloom = BloomFilter::new(MB * 8, 64);

        b.iter(|| bloom.insert(black_box(&())));
    });

    c.bench_function("u64", |b| {
        let mut bloom = BloomFilter::new(MB * 8, 64);
        let mut counter: u64 = 0;

        b.iter(|| {
            bloom.insert(black_box(&counter));
            counter = counter.wrapping_add(1);
        })
    });

    c.bench_function("small_slice", |b| {
        let mut bloom = BloomFilter::new(MB * 8, 64);
        let mut slice = vec![77u64; 64];

        b.iter(|| {
            bloom.insert(black_box(&slice));
            slice[7] = slice[7].wrapping_add(77);
        });
    });

    c.bench_function("large_slice", |b| {
        let mut bloom = BloomFilter::new(MB * 8, 64);
        let mut slice = vec![77u64; 16384];

        b.iter(|| {
            bloom.insert(black_box(&slice));
            slice[7] = slice[7].wrapping_add(77);
            slice[9001] = slice[9001].wrapping_add(707070);
        });
    });
}

criterion_group!(benches, small_key);
criterion_main!(benches);
