// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use super::*;
use crate::hashtable::HashBucket;
use crate::item::ITEM_HDR_SIZE;
use core::num::NonZeroU32;

use std::time::Duration;

#[test]
fn sizes() {
    #[cfg(feature = "magic")]
    assert_eq!(ITEM_HDR_SIZE, 9);

    #[cfg(not(feature = "magic"))]
    assert_eq!(ITEM_HDR_SIZE, 5);

    assert_eq!(std::mem::size_of::<Segments>(), 64);
    assert_eq!(std::mem::size_of::<SegmentHeader>(), 64);

    assert_eq!(std::mem::size_of::<HashBucket>(), 64);
    assert_eq!(std::mem::size_of::<HashTable>(), 64);

    assert_eq!(std::mem::size_of::<crate::ttl_buckets::TtlBucket>(), 64);
    assert_eq!(std::mem::size_of::<TtlBuckets>(), 24);
}

#[test]
fn init() {
    let mut cache = Seg::builder()
        .segment_size(4096)
        .heap_size(4096 * 64)
        .build()
        .expect("failed to create cache");
    assert_eq!(cache.items(), 0);
}

#[test]
fn get_free_seg() {
    let segment_size = 4096;
    let segments = 64;
    let heap_size = segments * segment_size as usize;

    let mut cache = Seg::builder()
        .segment_size(segment_size)
        .heap_size(heap_size)
        .build()
        .expect("failed to create cache");
    assert_eq!(cache.items(), 0);
    assert_eq!(cache.segments.free(), 64);
    let seg = cache.segments.pop_free();
    assert_eq!(cache.segments.free(), 63);
    assert_eq!(seg, NonZeroU32::new(1));
}

#[test]
fn get() {
    let ttl = Duration::ZERO;
    let segment_size = 4096;
    let segments = 64;
    let heap_size = segments * segment_size as usize;

    let mut cache = Seg::builder()
        .segment_size(segment_size)
        .heap_size(heap_size)
        .build()
        .expect("failed to create cache");
    assert_eq!(cache.items(), 0);
    assert_eq!(cache.segments.free(), 64);
    assert!(cache.get(b"coffee").is_none());
    assert!(cache.insert(b"coffee", b"strong", None, ttl).is_ok());
    assert_eq!(cache.segments.free(), 63);
    assert_eq!(cache.items(), 1);
    assert!(cache.get(b"coffee").is_some());

    let item = cache.get(b"coffee").unwrap();
    assert_eq!(item.value(), b"strong", "item is: {:?}", item);
}

#[test]
fn cas() {
    let ttl = Duration::ZERO;
    let segment_size = 4096;
    let segments = 64;
    let heap_size = segments * segment_size as usize;

    let mut cache = Seg::builder()
        .segment_size(segment_size)
        .heap_size(heap_size)
        .build()
        .expect("failed to create cache");
    assert_eq!(cache.items(), 0);
    assert_eq!(cache.segments.free(), 64);
    assert!(cache.get(b"coffee").is_none());
    assert_eq!(
        cache.cas(b"coffee", b"hot", None, ttl, 0),
        Err(SegError::NotFound)
    );
    assert!(cache.insert(b"coffee", b"hot", None, ttl).is_ok());
    assert_eq!(
        cache.cas(b"coffee", b"iced", None, ttl, 0),
        Err(SegError::Exists)
    );
    let item = cache.get(b"coffee").unwrap();
    assert_eq!(cache.cas(b"coffee", b"iced", None, ttl, item.cas()), Ok(()));
}

#[test]
fn overwrite() {
    let ttl = Duration::ZERO;
    let segment_size = 4096;
    let segments = 64;
    let heap_size = segments * segment_size as usize;

    let mut cache = Seg::builder()
        .segment_size(segment_size)
        .heap_size(heap_size)
        .build()
        .expect("failed to create cache");
    assert_eq!(cache.items(), 0);
    assert_eq!(cache.segments.free(), 64);
    assert!(cache.get(b"drink").is_none());

    println!("==== first insert ====");
    assert!(cache.insert(b"drink", b"coffee", None, ttl).is_ok());
    assert_eq!(cache.segments.free(), 63);
    assert_eq!(cache.items(), 1);
    let item = cache.get(b"drink");
    assert!(item.is_some());
    let item = item.unwrap();
    let value = item.value();
    assert_eq!(value, b"coffee", "item is: {:?}", item);

    println!("==== second insert ====");
    assert!(cache.insert(b"drink", b"espresso", None, ttl).is_ok());
    assert_eq!(cache.segments.free(), 63);
    assert_eq!(cache.items(), 1);
    let item = cache.get(b"drink");
    assert!(item.is_some());
    let item = item.unwrap();
    let value = item.value();
    assert_eq!(value, b"espresso", "item is: {:?}", item);

    println!("==== third insert ====");
    assert!(cache.insert(b"drink", b"whisky", None, ttl).is_ok());
    assert_eq!(cache.segments.free(), 63);
    assert_eq!(cache.items(), 1);
    let item = cache.get(b"drink");
    assert!(item.is_some());
    let item = item.unwrap();
    let value = item.value();
    assert_eq!(value, b"whisky", "item is: {:?}", item);
}

#[test]
fn delete() {
    let ttl = Duration::ZERO;
    let segment_size = 4096;
    let segments = 64;
    let heap_size = segments * segment_size as usize;

    let mut cache = Seg::builder()
        .segment_size(segment_size)
        .heap_size(heap_size)
        .build()
        .expect("failed to create cache");
    assert_eq!(cache.items(), 0);
    assert_eq!(cache.segments.free(), 64);
    assert!(cache.get(b"drink").is_none());

    assert!(cache.insert(b"drink", b"coffee", None, ttl).is_ok());
    assert_eq!(cache.segments.free(), 63);
    assert_eq!(cache.items(), 1);
    let item = cache.get(b"drink");
    assert!(item.is_some());
    let item = item.unwrap();
    let value = item.value();
    assert_eq!(value, b"coffee", "item is: {:?}", item);

    assert_eq!(cache.delete(b"drink"), true);
    assert_eq!(cache.segments.free(), 63);
    assert_eq!(cache.items(), 0);
}

#[test]
fn collisions_2() {
    let ttl = Duration::ZERO;
    let segment_size = 64;
    let segments = 2;
    let heap_size = segments * segment_size as usize;

    let mut cache = Seg::builder()
        .segment_size(segment_size)
        .heap_size(heap_size)
        .hash_power(3)
        .build()
        .expect("failed to create cache");
    assert_eq!(cache.items(), 0);
    assert_eq!(cache.segments.free(), 2);

    // note: we can only fit 7 because the first bucket in the chain only
    // has 7 slots. since we don't support chaining, we must have a
    // collision on the 8th insert.
    for i in 0..1000 {
        let i = i % 3;
        let v = format!("{:02}", i);
        assert!(cache.insert(v.as_bytes(), v.as_bytes(), None, ttl).is_ok());
        let item = cache.get(v.as_bytes());
        assert!(item.is_some());
    }
}

#[test]
fn collisions() {
    let ttl = Duration::ZERO;
    let segment_size = 4096;
    let segments = 64;
    let heap_size = segments * segment_size as usize;

    let mut cache = Seg::builder()
        .segment_size(segment_size)
        .heap_size(heap_size)
        .hash_power(3)
        .build()
        .expect("failed to create cache");
    assert_eq!(cache.items(), 0);
    assert_eq!(cache.segments.free(), 64);

    // note: we can only fit 7 because the first bucket in the chain only
    // has 7 slots. since we don't support chaining, we must have a
    // collision on the 8th insert.
    for i in 0..7 {
        let v = format!("{}", i);
        assert!(cache.insert(v.as_bytes(), v.as_bytes(), None, ttl).is_ok());
        let item = cache.get(v.as_bytes());
        assert!(item.is_some());
        assert_eq!(cache.items(), i + 1);
    }
    let v = b"8";
    assert!(cache.insert(v, v, None, ttl).is_err());
    assert_eq!(cache.items(), 7);
    assert_eq!(cache.delete(b"0"), true);
    assert_eq!(cache.items(), 6);
    assert!(cache.insert(v, v, None, ttl).is_ok());
    assert_eq!(cache.items(), 7);
}

#[test]
fn full_cache_long() {
    let ttl = Duration::ZERO;
    let iters = 1_000_000;
    let segments = 32;
    let segment_size = 1024;
    let key_size = 1;
    let value_size = 512;
    let heap_size = segments * segment_size as usize;

    let mut cache = Seg::builder()
        .segment_size(segment_size)
        .heap_size(heap_size)
        .hash_power(16)
        .build()
        .expect("failed to create cache");

    assert_eq!(cache.items(), 0);
    assert_eq!(cache.segments.free(), segments);

    let mut rng = rand::rng();

    let mut key = vec![0; key_size];
    let mut value = vec![0; value_size];

    let mut inserts = 0;

    for _ in 0..iters {
        rng.fill_bytes(&mut key);
        rng.fill_bytes(&mut value);

        if cache.insert(&key, &value, None, ttl).is_ok() {
            inserts += 1;
        };
    }

    assert_eq!(inserts, iters);
}

#[test]
fn full_cache_long_2() {
    let ttl = Duration::ZERO;
    let iters = 10_000_000;
    let segments = 64;
    let segment_size = 2 * 1024;
    let key_size = 2;
    let value_size = 1;
    let heap_size = segments * segment_size as usize;

    let mut cache = Seg::builder()
        .segment_size(segment_size)
        .heap_size(heap_size)
        .hash_power(16)
        .build()
        .expect("failed to create cache");

    assert_eq!(cache.items(), 0);
    assert_eq!(cache.segments.free(), segments);

    let mut rng = rand::rng();

    let mut key = vec![0; key_size];
    let mut value = vec![0; value_size];

    let mut inserts = 0;

    for _ in 0..iters {
        rng.fill_bytes(&mut key);
        rng.fill_bytes(&mut value);

        if cache.insert(&key, &value, None, ttl).is_ok() {
            inserts += 1;
        };
    }

    // inserts should be > 99.99 percent successful for this config
    assert!(inserts >= 9_999_000);
}

#[test]
fn expiration() {
    let segments = 64;
    let segment_size = 2 * 1024;
    let heap_size = segments * segment_size as usize;

    let mut cache = Seg::builder()
        .segment_size(segment_size)
        .heap_size(heap_size)
        .hash_power(16)
        .build()
        .expect("failed to create cache");

    assert_eq!(cache.items(), 0);
    assert_eq!(cache.segments.free(), segments);

    assert!(cache
        .insert(b"latte", b"", None, Duration::from_secs(5))
        .is_ok());
    assert!(cache
        .insert(b"espresso", b"", None, Duration::from_secs(15))
        .is_ok());

    assert!(cache.get(b"latte").is_some());
    assert!(cache.get(b"espresso").is_some());
    assert_eq!(cache.items(), 2);
    assert_eq!(cache.segments.free(), segments - 2);

    // not enough time elapsed, not removed by expire
    cache.expire();
    assert!(cache.get(b"latte").is_some());
    assert!(cache.get(b"espresso").is_some());
    assert_eq!(cache.items(), 2);
    assert_eq!(cache.segments.free(), segments - 2);

    // wait and expire again
    std::thread::sleep(std::time::Duration::from_secs(5));
    cache.expire();

    assert!(cache.get(b"latte").is_none());
    assert!(cache.get(b"espresso").is_some());
    assert_eq!(cache.items(), 1);
    assert_eq!(cache.segments.free(), segments - 1);

    // wait and expire again
    std::thread::sleep(std::time::Duration::from_secs(10));
    cache.expire();

    assert!(cache.get(b"latte").is_none());
    assert!(cache.get(b"espresso").is_none());
    assert_eq!(cache.items(), 0);
    assert_eq!(cache.segments.free(), segments);
}

#[test]
fn clear() {
    let ttl = Duration::ZERO;
    let segment_size = 4096;
    let segments = 64;
    let heap_size = segments * segment_size as usize;

    let mut cache = Seg::builder()
        .segment_size(segment_size)
        .heap_size(heap_size)
        .build()
        .expect("failed to create cache");
    assert_eq!(cache.items(), 0);
    assert_eq!(cache.segments.free(), segments);
    assert!(cache.get(b"coffee").is_none());
    assert!(cache.insert(b"coffee", b"strong", None, ttl).is_ok());
    assert_eq!(cache.segments.free(), segments - 1);
    assert_eq!(cache.items(), 1);
    assert!(cache.get(b"coffee").is_some());

    let item = cache.get(b"coffee").unwrap();
    assert_eq!(item.value(), b"strong", "item is: {:?}", item);

    cache.clear();
    assert_eq!(cache.segments.free(), segments);
    assert_eq!(cache.items(), 0);
    assert!(cache.get(b"coffee").is_none());
}

#[test]
fn wrapping_add() {
    let ttl = Duration::ZERO;
    let segment_size = 4096;
    let segments = 64;
    let heap_size = segments * segment_size as usize;

    let mut cache = Seg::builder()
        .segment_size(segment_size)
        .heap_size(heap_size)
        .build()
        .expect("failed to create cache");
    assert_eq!(cache.items(), 0);
    assert_eq!(cache.segments.free(), 64);
    assert!(cache.insert(b"coffee", 0, None, ttl).is_ok());
    assert_eq!(cache.segments.free(), 63);
    assert_eq!(cache.items(), 1);
    assert!(cache.get(b"coffee").is_some());

    let item = cache.get(b"coffee").unwrap();
    assert_eq!(item.value(), 0, "item is: {:?}", item);
    cache
        .wrapping_add(b"coffee", 1)
        .expect("failed to increment");
    assert_eq!(item.value(), 1, "item is: {:?}", item);
    cache
        .wrapping_add(b"coffee", u64::MAX - 1)
        .expect("failed to increment");
    assert_eq!(item.value(), u64::MAX, "item is: {:?}", item);
    cache
        .wrapping_add(b"coffee", 1)
        .expect("failed to increment");
    assert_eq!(item.value(), 0, "item is: {:?}", item);
    cache
        .wrapping_add(b"coffee", 2)
        .expect("failed to increment");
    assert_eq!(item.value(), 2, "item is: {:?}", item);
}

#[test]
fn saturating_sub() {
    let ttl = Duration::ZERO;
    let segment_size = 4096;
    let segments = 64;
    let heap_size = segments * segment_size as usize;

    let mut cache = Seg::builder()
        .segment_size(segment_size)
        .heap_size(heap_size)
        .build()
        .expect("failed to create cache");
    assert_eq!(cache.items(), 0);
    assert_eq!(cache.segments.free(), 64);
    assert!(cache.insert(b"coffee", 3, None, ttl).is_ok());
    assert_eq!(cache.segments.free(), 63);
    assert_eq!(cache.items(), 1);
    assert!(cache.get(b"coffee").is_some());

    let item = cache.get(b"coffee").unwrap();
    assert_eq!(item.value(), 3, "item is: {:?}", item);
    cache
        .saturating_sub(b"coffee", 2)
        .expect("failed to increment");
    assert_eq!(item.value(), 1, "item is: {:?}", item);
    cache
        .saturating_sub(b"coffee", 1)
        .expect("failed to increment");
    assert_eq!(item.value(), 0, "item is: {:?}", item);
    cache
        .saturating_sub(b"coffee", 1)
        .expect("failed to increment");
    assert_eq!(item.value(), 0, "item is: {:?}", item);
}

#[test]
// This test caught a case where we interpreted old data as part of an item
// header. Specifically, the first insert sets bytes that will be in-range for
// the item header for the third insert. This happens to set the "typed value"
// bit, which stopped the item definition from setting the value length. This
// caused the item value to be invalid. Triggering a removal of this item with
// the corrupted length caused a panic on the asserts, which correctly detected
// the bad state.
fn fuzz_1() {
    let mut cache = Seg::builder()
        .segment_size(1024)
        .heap_size(8 * 1024)
        .hash_power(3)
        .overflow_factor(0.0)
        .build()
        .expect("failed to create cache");

    let _ = cache.insert(
        &[
            195, 195, 195, 195, 195, 195, 195, 195, 195, 195, 195, 195, 195, 195, 195, 195, 195,
            195, 195, 195, 195, 195, 195, 195, 195, 195, 195, 195, 195, 195, 195, 195, 195, 195,
            195, 195, 195, 195, 195, 195, 195, 195, 195, 195, 195, 195, 195, 195, 195, 195, 195,
            195, 195, 195, 195, 195, 195, 195, 195, 195, 195, 195, 195, 195, 195, 195, 195, 195,
            195, 195, 195, 195, 195, 195, 195, 195, 195, 195, 195, 195, 195, 195, 195, 19, 5, 195,
            195, 195, 195, 195, 195, 195, 195, 195, 4, 0, 4, 2, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
            4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
            4, 4, 4, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 1, 3, 0, 1, 0, 4, 181, 10, 4, 4, 4, 4, 4, 4,
            4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 59, 8, 4,
        ],
        &[4, 4, 4, 4],
        None,
        Duration::from_secs(0),
    );

    let _ = cache.clear();
    assert_eq!(cache.items(), 0);

    let _ = cache.insert(&[1], &[0xDE, 0xAD, 0xBE, 0xEF], None, Duration::from_secs(4));
    let _ = cache.insert(&[1], &[0xC0, 0xFF, 0xEE], None, Duration::from_secs(2));
    let _ = cache.delete(&[1]);
}

#[test]
// This test found an issue when freeing a segment because its live item count
// dropped to zero. This is a more complicated way of triggering the same
// behavior as fuzz_1 test, but also exposed that we had a tracking issue for
// dead bytes when recycling a segment when live items dropped to zero.
fn fuzz_2() {
    let mut cache = Seg::builder()
        .segment_size(1024)
        .heap_size(8 * 1024)
        .hash_power(5)
        .overflow_factor(1.0)
        .build()
        .expect("failed to create cache");

    let _ = cache.insert(&[1], &[3, 4, 2], None, Duration::from_secs(0));
    let _ = cache.insert(&[4, 0, 4, 48], &[], None, Duration::from_secs(0));
    let _ = cache.insert(&[1], &[3, 0, 1], None, Duration::from_secs(0));
    let _ = cache.insert(&[4, 0, 4, 48], &[], None, Duration::from_secs(0));
    let _ = cache.insert(&[1], &[3, 0, 1], None, Duration::from_secs(0));
    let _ = cache.insert(&[1], &[3, 3, 0], None, Duration::from_secs(4));
    let _ = cache.insert(&[1], &[3, 4, 2], None, Duration::from_secs(0));
    let _ = cache.insert(&[4, 0, 4, 48], &[], None, Duration::from_secs(0));
    let _ = cache.insert(&[2], &[], None, Duration::from_secs(0));
    let _ = cache.insert(&[4, 0, 4, 48], &[], None, Duration::from_secs(0));
    let _ = cache.insert(&[1], &[3, 0, 1], None, Duration::from_secs(0));
    let _ = cache.insert(
        &[
            81, 0, 0, 0, 1, 0, 10, 0, 1, 3, 0, 1, 0, 1, 0, 3, 0, 1, 3, 0, 1, 0, 4, 2, 114, 0, 4, 0,
            4, 48, 0, 0, 3, 4, 10, 1, 3, 0, 1, 3, 0, 237, 237, 237, 237, 237, 237, 237, 237, 237,
            237, 237, 237, 237, 237, 237, 237, 237, 228, 237, 237, 237, 237, 237, 237, 237, 237,
            237, 237, 237, 237, 237, 237, 237, 237, 237, 237, 237, 237, 237, 1,
        ],
        &[],
        None,
        Duration::from_secs(0),
    );
    let _ = cache.insert(&[1], &[], None, Duration::from_secs(0));
    let _ = cache.insert(
        &[
            228, 3, 0, 1, 0, 4, 2, 114, 0, 4, 0, 4, 48, 0, 0, 3, 4, 10, 1, 3, 0, 1, 3, 0, 1, 0, 4,
            2, 1, 0, 1, 3, 4, 2, 114, 0, 4, 0, 4, 48, 0, 0, 3, 4, 10, 1, 3, 0, 1, 3, 0, 1, 0, 4, 2,
            114, 0, 4, 0, 4, 48, 0, 0, 3, 4, 10, 1, 3, 0, 1, 3, 0, 1, 0, 18, 255, 1, 0, 0, 1, 0, 2,
            4, 1, 1, 1, 1, 1, 1, 1, 1, 101, 0, 0, 0, 1, 0, 10, 0, 1, 3, 0, 1, 0, 1, 0, 3, 0, 1, 3,
            0, 1, 0, 4, 2, 114, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
            255, 255, 255, 255, 0, 4, 0, 4, 48, 0, 0, 255, 1, 0, 0, 1, 0, 2, 4, 1, 1, 1, 2, 2, 2,
            2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 1, 0, 4,
            2, 1, 0, 1, 3, 4, 2, 114, 0, 4, 0, 4, 48, 0, 0, 3, 4, 10, 1, 3, 0, 1, 3, 0, 1, 0, 4, 2,
            114, 0, 4, 0, 4, 48, 0, 0, 3, 4, 10, 1, 3, 0, 1, 3, 0,
        ],
        &[1],
        None,
        Duration::from_secs(0),
    );
    let _ = cache.insert(&[4, 0, 4, 48], &[], None, Duration::from_secs(0));
    let _ = cache.insert(&[3, 4, 0], &[3, 1, 0], None, Duration::from_secs(10));
    let _ = cache.delete(&[3, 1, 0]);
    let _ = cache.insert(&[4, 0, 4, 48], &[], None, Duration::from_secs(0));
    let _ = cache.insert(&[1], &[3, 0, 1], None, Duration::from_secs(0));
    let _ = cache.insert(
        &[
            81, 0, 0, 0, 1, 0, 10, 0, 1, 3, 0, 1, 0, 1, 0, 3, 0, 1, 3, 0, 1, 0, 4, 2, 114, 0, 4, 0,
            4, 48, 0, 0, 3, 4, 10, 1, 3, 0, 1, 3, 0, 237, 237, 237, 237, 237, 237, 237, 237, 237,
            237, 237, 237, 237, 237, 237, 237, 237, 228, 237, 237, 237, 237, 237, 237, 237, 237,
            237, 237, 237, 237, 237, 237, 237, 237, 237, 237, 237, 237, 237, 1,
        ],
        &[],
        None,
        Duration::from_secs(0),
    );
    let _ = cache.insert(
        &[
            228, 3, 0, 1, 0, 4, 2, 114, 0, 4, 0, 4, 48, 0, 0, 3, 4, 2, 1, 3, 0, 1, 3, 0, 1, 0, 4,
            2, 1, 0, 1, 3, 4, 2, 114, 0, 4, 0, 4, 48, 0, 0, 3, 4, 10, 1, 3, 0, 1, 3, 0, 1, 0, 4, 2,
            114, 0, 4, 0, 4, 48, 0, 0, 3, 4, 10, 1, 3, 0, 1, 3, 0, 1, 0, 4, 2, 1, 0, 1, 3, 3, 0, 4,
            0, 1, 3, 0, 4, 1, 0, 81, 0, 0, 0, 1, 0, 10, 81, 0, 0, 0, 1, 0, 1, 0, 3, 0, 1, 3, 0, 1,
            1, 0, 1, 3, 4, 2, 116, 0, 2, 2, 255, 255, 0, 3, 1, 0, 2, 0, 0, 0, 3, 4, 10, 4, 2, 114,
            0, 4, 0, 4, 48, 0, 0, 3, 4, 10, 1, 5, 0, 255, 252, 255, 254, 255, 251, 2, 114, 0, 4, 4,
            4, 0, 1, 1, 2, 1, 1, 0, 1, 1, 2, 1, 1, 0, 1, 2, 1, 1, 0, 1, 1, 1, 0, 1, 1, 2, 1, 1, 0,
            1, 1, 1, 0, 1, 0, 4, 48, 0, 0, 3, 4, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
            255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
        ],
        &[
            255, 255, 255, 255, 10, 1, 3, 0, 1, 3, 0, 1, 0, 4, 2, 1, 0, 1, 3, 3, 0, 4, 0, 1, 3, 0,
            4, 1, 0, 81, 0, 0, 0, 1, 0, 10, 0, 1, 3, 0, 1, 0, 255, 255, 255, 255, 0, 4, 0, 4, 48,
            0, 0, 255, 1, 0, 0, 1, 0, 2, 4, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
            2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 1, 0, 4, 2, 1, 0, 1, 3, 4, 2, 114, 0, 4, 0,
            4, 48, 0, 0, 3, 4, 10, 1, 3, 0, 1, 3, 0, 1, 0, 4, 2, 114, 0, 4, 0, 4, 48, 0, 0, 3, 4,
            10, 1, 3, 0, 1, 3, 0, 1, 0, 4, 2, 1, 0, 1, 3, 3, 0, 4, 0, 1, 3, 0, 4, 1, 0, 81, 0, 0,
            0, 1, 0, 10, 0, 1, 3, 0, 1, 0, 1, 0, 3, 0, 1, 3, 0, 1, 0, 4, 2, 114, 0, 4, 0, 4, 48, 0,
            0, 3, 4, 10, 1, 3, 0, 1, 3, 0, 237, 237, 237, 237, 237, 237, 237, 237, 237, 237, 237,
            237, 237, 237, 237, 237, 237, 228, 237, 237, 237, 237, 237, 237, 237, 237, 237, 237,
            237, 237, 237, 237, 237, 237, 237, 237, 237, 237, 237, 1, 0, 0, 228, 3, 0, 1, 0, 4, 2,
            114, 0, 4, 0, 4, 48,
        ],
        None,
        Duration::from_secs(0),
    );
    let _ = cache.insert(&[1], &[3, 0, 1], None, Duration::from_secs(0));
    let _ = cache.insert(&[1], &[3, 4, 2], None, Duration::from_secs(114));
    let _ = cache.insert(&[4, 0, 4, 48], &[], None, Duration::from_secs(0));
    let _ = cache.insert(&[1], &[3, 0, 1], None, Duration::from_secs(0));
    let _ = cache.insert(&[4, 0, 4, 48], &[], None, Duration::from_secs(0));
    let _ = cache.insert(&[1], &[3, 0, 1], None, Duration::from_secs(0));
    let _ = cache.insert(&[3], &[3, 4, 2], None, Duration::from_secs(114));
    let _ = cache.insert(&[4, 0, 4, 48], &[], None, Duration::from_secs(0));
    let _ = cache.insert(&[2], &[], None, Duration::from_secs(0));
    let _ = cache.insert(&[3, 4, 0], &[3, 1, 0], None, Duration::from_secs(10));
    let _ = cache.delete(&[3, 1, 0]);
    let _ = cache.insert(&[4, 0, 4, 48], &[], None, Duration::from_secs(0));
    let _ = cache.insert(&[1], &[3, 0, 1], None, Duration::from_secs(0));
    let _ = cache.insert(
        &[
            81, 0, 0, 0, 1, 0, 10, 0, 1, 3, 0, 1, 0, 1, 0, 3, 0, 1, 3, 0, 1, 0, 4, 2, 114, 0, 4, 0,
            4, 48, 0, 0, 3, 4, 10, 1, 3, 0, 1, 3, 0, 237, 237, 237, 237, 237, 237, 237, 237, 237,
            237, 237, 237, 237, 237, 237, 237, 237, 228, 237, 237, 237, 237, 237, 237, 237, 237,
            237, 237, 237, 237, 237, 237, 237, 237, 237, 237, 237, 237, 237, 1,
        ],
        &[],
        None,
        Duration::from_secs(0),
    );
    let _ = cache.insert(
        &[
            228, 3, 0, 1, 0, 4, 2, 114, 0, 4, 0, 4, 48, 0, 0, 3, 4, 10, 1, 3, 0, 1, 3, 0, 1, 0, 4,
            2, 1, 0, 1, 3, 4, 2, 114, 0, 4, 0, 4, 48, 0, 0, 3, 4, 10, 1, 3, 0, 1, 3, 0, 1, 0, 4, 2,
            114, 0, 4, 0, 4, 48, 0, 0, 3, 4, 10, 1, 3, 0, 1, 3, 0, 1, 0, 4, 2, 1, 0, 1, 3, 3, 0, 4,
            0, 1, 3, 0, 4, 1, 0, 81, 0, 0, 0, 1, 0, 10, 81, 0, 0, 0, 1, 0, 1, 0, 3, 0, 1, 3, 0, 1,
            1, 0, 1, 3, 4, 2, 116, 0, 2, 2, 255, 255, 0, 3, 1, 0, 2, 0, 0, 0, 3, 4, 10, 4, 2, 114,
            0, 4, 0, 4, 48, 0, 0, 3, 4, 10, 1, 3, 0, 1, 3, 0, 1, 0, 4, 2, 114, 0, 4, 0, 4, 48, 0,
            0, 3, 4, 10, 1, 3, 0, 1, 3, 0, 1, 0, 4, 2, 1, 0, 1, 3, 3, 0, 4, 0, 1, 3, 0, 4, 1, 0,
            81, 0, 0, 0, 1, 0, 10, 0, 1, 3, 0, 1, 0, 1, 0, 3, 0, 1, 3, 0, 1, 0, 4, 2, 114, 0, 4, 0,
            4, 48, 0, 0, 3, 4, 10, 1,
        ],
        &[3, 0, 1],
        None,
        Duration::from_secs(3),
    );
    let _ = cache.insert(&[1], &[], None, Duration::from_secs(0));
    let _ = cache.insert(
        &[
            228, 3, 0, 1, 0, 4, 2, 114, 0, 4, 0, 4, 48, 0, 0, 3, 4, 10, 1, 3, 0, 1, 3, 0, 1, 0, 4,
            2, 1, 0, 1, 3, 4, 2, 114, 0, 4, 0, 4, 48, 0, 0, 3, 4, 10, 1, 3, 0, 1, 3, 0, 1, 0, 4, 2,
            114, 0, 4, 0, 4, 48, 0, 0, 3, 4, 10, 1, 3, 0, 1, 3, 0, 1, 0, 18, 255, 1, 0, 0, 1, 0, 2,
            4, 1, 1, 1, 1, 1, 1, 1, 1, 101, 0, 0, 0, 1, 0, 10, 0, 1, 3, 0, 1, 0, 1, 0, 3, 0, 1, 3,
            0, 1, 0, 4, 2, 114, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
            255, 255, 255, 255, 0, 4, 0, 4, 48, 0, 0, 255, 1, 0, 0, 1, 0, 2, 4, 1, 1, 1, 2, 2, 2,
            2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 1, 0, 4,
            2, 1, 0, 1, 3, 4, 2, 114, 0, 4, 0, 4, 48, 0, 0, 3, 4, 10, 1, 3, 0, 1, 3, 0, 1, 0, 4, 2,
            114, 0, 4, 0, 4, 48, 0, 0, 3, 4, 10, 1, 3, 0, 1, 3, 0,
        ],
        &[1],
        None,
        Duration::from_secs(0),
    );
    let _ = cache.insert(&[4, 0, 4, 48], &[], None, Duration::from_secs(0));
    let _ = cache.insert(&[3, 4, 0], &[3, 1, 0], None, Duration::from_secs(10));
    let _ = cache.delete(&[3, 1, 0]);
    let _ = cache.insert(&[4, 0, 4, 48], &[], None, Duration::from_secs(0));
    let _ = cache.insert(&[1], &[3, 0, 1], None, Duration::from_secs(0));
    let _ = cache.insert(
        &[
            81, 0, 0, 0, 1, 0, 10, 0, 1, 3, 0, 1, 0, 1, 0, 3, 0, 1, 3, 0, 1, 0, 4, 2, 114, 0, 4, 0,
            4, 48, 0, 0, 3, 4, 10, 1, 3, 0, 1, 3, 0, 237, 237, 237, 237, 237, 237, 237, 237, 237,
            237, 237, 237, 237, 237, 237, 237, 237, 228, 237, 237, 237, 237, 237, 237, 237, 237,
            237, 237, 237, 237, 237, 237, 237, 237, 237, 237, 237, 237, 237, 1,
        ],
        &[],
        None,
        Duration::from_secs(0),
    );
    let _ = cache.insert(
        &[
            228, 3, 0, 1, 0, 4, 2, 114, 0, 4, 0, 4, 48, 0, 0, 3, 4, 2, 1, 3, 0, 1, 3, 0, 1, 0, 4,
            2, 1, 0, 1, 3, 4, 2, 114, 0, 4, 0, 4, 48, 0, 0, 3, 4, 10, 1, 3, 0, 1, 3, 0, 1, 0, 4, 2,
            114, 0, 4, 0, 4, 48, 0, 0, 3, 4, 10, 1, 3, 0, 1, 3, 0, 1, 0, 4, 2, 1, 0, 1, 3, 3, 0, 4,
            0, 1, 3, 0, 4, 1, 0, 81, 0, 0, 0, 1, 0, 10, 81, 0, 0, 0, 1, 0, 1, 0, 3, 0, 1, 3, 0, 1,
            1, 0, 1, 3, 4, 2, 116, 0, 2, 2, 255, 255, 0, 3, 1, 0, 2, 0, 0, 0, 3, 4, 10, 4, 2, 114,
            0, 4, 0, 4, 48, 0, 0, 3, 4, 10, 1, 5, 0, 255, 252, 255, 254, 255, 251, 2, 114, 0, 4, 4,
            4, 0, 1, 1, 2, 1, 1, 0, 1, 1, 2, 1, 1, 0, 1, 2, 1, 1, 0, 1, 1, 1, 0, 1, 1, 2, 1, 1, 0,
            1, 1, 1, 0, 1, 0, 4, 48, 0, 0, 3, 4, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
            255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
        ],
        &[
            255, 255, 255, 255, 10, 1, 3, 0, 1, 3, 0, 1, 0, 4, 2, 1, 0, 1, 3, 3, 0, 4, 0, 1, 3, 0,
            4, 1, 0, 81, 0, 0, 0, 1, 0, 10, 0, 1, 3, 0, 1, 0, 255, 255, 255, 255, 0, 4, 0, 4, 48,
            0, 0, 255, 1, 0, 0, 1, 0, 2, 4, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
            2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 1, 0, 4, 2, 1, 0, 1, 3, 4, 2, 114, 0, 4, 0,
            4, 48, 0, 0, 3, 4, 10, 1, 3, 0, 1, 3, 0, 1, 0, 4, 2, 114, 0, 4, 0, 4, 48, 0, 0, 3, 4,
            10, 1, 3, 0, 1, 3, 0, 1, 0, 4, 2, 1, 0, 1, 3, 3, 0, 4, 0, 1, 3, 0, 4, 1, 0, 81, 0, 0,
            0, 1, 0, 10, 0, 1, 3, 0, 1, 0, 1, 0, 3, 0, 1, 3, 0, 1, 0, 4, 2, 114, 0, 4, 0, 4, 48, 0,
            0, 3, 4, 10, 1, 3, 0, 1, 3, 0, 237, 237, 237, 237, 237, 237, 237, 237, 237, 237, 237,
            237, 237, 237, 237, 237, 237, 228, 237, 237, 237, 237, 237, 237, 237, 237, 237, 237,
            237, 237, 237, 237, 237, 237, 237, 237, 237, 237, 237, 1, 0, 0, 228, 3, 0, 1, 0, 4, 2,
            114, 0, 4, 0, 4, 48,
        ],
        None,
        Duration::from_secs(0),
    );
    let _ = cache.insert(&[1], &[3, 0, 1], None, Duration::from_secs(0));
    let _ = cache.insert(&[1], &[3, 4, 2], None, Duration::from_secs(114));
}
