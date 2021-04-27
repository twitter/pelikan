use rand::RngCore;
use rand::SeedableRng;
use segcache::*;
use std::time::Instant;

pub const MB: usize = 1024 * 1024;

fn main() {
    let build_hasher = ahash::RandomState::with_seeds(
        0xbb8c484891ec6c86,
        0x0522a25ae9c769f9,
        0xeed2797b9571bc75,
        0x4feb29c1fbbd59d0,
    );

    let mut cache = SegCache::builder()
        .heap_size(128 * MB)
        .segment_size(MB as i32)
        .power(22)
        .hasher(build_hasher)
        .build();

    let (keys, values) = key_values(5, 1_000_000, 1, 1_000_000);

    let mut key = 0;
    let mut value = 0;
    let ttl = rustcommon_time::CoarseDuration::ZERO;

    let iters = 100_000_000;

    let start = Instant::now();
    for _ in 0..iters {
        let _ = cache.insert(&keys[key], &values[value], None, ttl);
        key += 1;
        if key >= keys.len() {
            key = 0;
        }
        value += 1;
        if value >= values.len() {
            value = 0;
        }
    }
    let stop = Instant::now();

    let rate = (iters as f64
        / ((stop - start).as_secs() as f64
            + ((stop - start).subsec_nanos() as f64 / 1_000_000_000.0))) as u64;
    println!("rate: {} insert/s", rate);
}

pub fn rng() -> impl RngCore {
    rand_xoshiro::Xoshiro256PlusPlus::seed_from_u64(0)
}

fn key_values(
    key_size: usize,
    key_count: usize,
    value_size: usize,
    value_count: usize,
) -> (Vec<Vec<u8>>, Vec<Vec<u8>>) {
    let mut rng = rng();

    let mut keys = Vec::new();
    for _ in 0..key_count {
        let mut key = vec![0; key_size];
        rng.fill_bytes(&mut key);
        keys.push(key);
    }

    let mut values = Vec::new();
    for _ in 0..value_count {
        let mut value = vec![0; value_size];
        rng.fill_bytes(&mut value);
        values.push(value);
    }

    (keys, values)
}
