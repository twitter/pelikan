// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use super::*;
use crate::hashtable::HashBucket;
use crate::item::ITEM_HDR_SIZE;
use core::num::NonZeroU32;
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Duration;
use tempfile::TempDir;

#[test]
fn sizes() {
    #[cfg(feature = "magic")]
    assert_eq!(ITEM_HDR_SIZE, 9);

    #[cfg(not(feature = "magic"))]
    assert_eq!(ITEM_HDR_SIZE, 5);

    assert_eq!(std::mem::size_of::<Segments>(), 64);
    assert_eq!(std::mem::size_of::<SegmentHeader>(), 64);

    assert_eq!(std::mem::size_of::<HashBucket>(), 64);
    assert_eq!(std::mem::size_of::<HashTable>(), 80);

    assert_eq!(std::mem::size_of::<crate::ttl_buckets::TtlBucket>(), 64);
    assert_eq!(std::mem::size_of::<TtlBuckets>(), 24);
}

#[test]
fn init() {
    let mut cache = Seg::builder()
        .segment_size(4096)
        .heap_size(4096 * 64)
        .build();
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
        .build();
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
        .build();
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
        .build();
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
        .build();
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
        .build();
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
        .build();
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
        .build();
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
        .build();

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
        .build();

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
        .build();

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
        .build();
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
        .build();
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
        .build();
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

// ----------- TESTS FOR RECOVERY -------------
// Configuration Options:
//
// New cache, not file backed
// ---- Cache is created new in main memory.
// New cache, file backed
// ---- Cache is created new and is file backed.
// ---- In other words, file is used as an extension of DRAM.
// ---- Note: The cache cannot be gracefully shutdown by if it isn't file backed.
// ---- That is, if there is no path used to file back the cache, there is no
// ---- path to copy the cache data to on shutdown
// Not gracefully shutdown
// ---- Nothing is saved on shutdown.
// Gracefully shutdown
// ---- `Segments.data` is flushed if it is file backed
// ---- Rest of `Seg` instance saved on shutdown if the `metadata_path` is valid.
// ---- That is, all of `Seg.hashtable`, `Seg.ttl_buckets` and the relevant
// ---- `Seg.Segments` fields are saved
// Restored cache
// ---- `Segments.data` must be file backed
// ---- Rest of `Seg` copied back from the files they were saved to and
// ---- If any of the file paths are not valid, then the cache is created new

// ------------- Set up / Helper Functions for below tests ------------

// path to tmp directory used for temp files
const TMP_DIR: &str = "target/debug/tmp";

const SEGMENTS: usize = 64;

// Creates a temporary directory for temporary test files
fn tmp_dir() -> TempDir {
    // Create parent directory for the temporary directory
    std::fs::create_dir_all(TMP_DIR).expect("failed to create parent tmp directory");

    // Create the temporary directory
    TempDir::new_in(TMP_DIR).unwrap()
}

// Returns a `Seg` instance. Cache is restored only if `restore` and
// `metadata_path` and `datapool_path` are not `None`. Otherwise, new `Seg`
// instance is returned. Cache is file backed if `datapool_path` is not `None`.
fn make_cache(
    restore: bool,
    datapool_path: Option<PathBuf>,
    metadata_path: Option<PathBuf>,
    graceful_shutdown: bool,
) -> Seg {
    let segment_size = 4096;
    let segments = SEGMENTS;
    let heap_size = segments * segment_size as usize;

    Seg::builder()
        .restore(restore)
        .segment_size(segment_size as i32)
        .heap_size(heap_size)
        .datapool_path(datapool_path) // set path
        .metadata_path(metadata_path) // set path
        .graceful_shutdown(graceful_shutdown)
        .build()
}

// ------------------- Set Paths Correctly Tests --------------------------

// Check that a file backed, new cache is file backed and the `Seg` and thus the
// `Segments` fields', `HashTable` and `TTLBuckets` are new (and not restored)
#[test]
fn new_cache_file_backed() {
    // Create parent directory for temporary test files
    let dir = tmp_dir();
    // Create tempfile for datapool
    let datapool_path: Option<PathBuf> = Some(dir.path().join("datapool"));

    // create new, file backed cache
    let restore = false;
    let graceful_shutdown = false;
    let cache = make_cache(restore, datapool_path, None, graceful_shutdown);

    // the `Segments.data` should be filed backed
    assert!(cache.segments.data_file_backed());
    // the `Seg` should not be restored
    assert!(!cache.restored());
}

// Check that a new, not file backed cache is not file backed and the `Seg` is
// new (and not restored)
#[test]
fn new_cache_not_file_backed() {
    // create new, not file backed cache
    let restore = false;
    let graceful_shutdown = false;
    let cache = make_cache(restore, None, None, graceful_shutdown);

    // the `Segments.data` should not be filed backed
    assert!(!cache.segments.data_file_backed());
    // the `Seg` should not be restored
    assert!(!cache.restored());
}

// Edge Case: Check that an attempt to restore a cache without specifing any
// paths will lead to `Segments.data` not being file backed and
// no of the other structures being restored
#[test]
fn restored_cache_no_paths_set() {
    let restore = true;
    let graceful_shutdown = false;
    let cache = make_cache(restore, None, None, graceful_shutdown);

    // the `Segments.data` should not be filed backed
    assert!(!cache.segments.data_file_backed());
    // the `Seg` should not be restored
    assert!(!cache.restored());
}

// Check that if paths are specified, then the cache is gracefully
// shutdown
#[test]
fn cache_gracefully_shutdown() {
    // Create a temporary directory
    let dir = tmp_dir();
    // Create tempfile for datapool
    let datapool_path: Option<PathBuf> = Some(dir.path().join("datapool"));
    // Create tempfile for `HashTable`
    let metadata_path: Option<PathBuf> = Some(dir.path().join("hashtable"));

    // create new, file backed cache
    let restore = false;
    let graceful_shutdown = true;
    let cache = make_cache(restore, datapool_path, metadata_path, graceful_shutdown);

    // Flush cache
    assert!(cache.flush().is_ok());
}

// Check that if paths are not specified, then the cache is not gracefully
// shutdown
#[test]
fn cache_not_gracefully_shutdown() {
    // Create a temporary directory
    let dir = tmp_dir();
    // Create tempfile for datapool
    let datapool_path: Option<PathBuf> = Some(dir.path().join("datapool"));

    // create new, file backed cache
    let restore = false;
    let graceful_shutdown = true;
    let cache = make_cache(
        restore,
        datapool_path,
        None, // Don't set a `HashTable` path
        graceful_shutdown,
    );

    // Flushing cache should fail
    assert!(cache.flush().is_err());
}

// --------------------- Data copied back Tests----------------------------

// Creates a new cache, stores an item, gracefully shutsdown cache and restore
// cache. Check item is still there and caches are equivalent
#[test]
fn new_file_backed_cache_changed_and_restored() {
    // Create a temporary directory
    let dir = tmp_dir();
    // Create tempfile for datapool
    let datapool_path: Option<PathBuf> = Some(dir.path().join("datapool"));
    // Create tempfile for `HashTable`
    let metadata_path: Option<PathBuf> = Some(dir.path().join("hashtable"));

    // create new, file backed cache
    let mut restore = false;
    let mut graceful_shutdown = true;
    let mut cache = make_cache(restore, datapool_path, metadata_path, graceful_shutdown);

    assert!(!cache.restored());
    assert_eq!(cache.items(), 0);
    assert_eq!(cache.segments.free(), SEGMENTS);

    // "latte" should not be in a new, empty cache
    assert!(cache.get(b"latte").is_none());
    // insert "latte" into cache
    assert!(cache
        .insert(b"latte", b"", None, Duration::from_secs(5))
        .is_ok());
    // "latte" should now be in cache
    assert!(cache.get(b"latte").is_some());

    assert_eq!(cache.items(), 1);
    assert_eq!(cache.segments.free(), SEGMENTS - 1);

    // Get a copy of the cache to be compared later
    let old_cache = cache.clone();

    // Flush cache
    assert!(cache.flush().is_ok());

    // Create same tempfiles (they have been moved since first created)
    let datapool_path: Option<PathBuf> = Some(dir.path().join("datapool"));
    let metadata_path: Option<PathBuf> = Some(dir.path().join("hashtable"));

    // restore cache
    // This cache is file backed by same file as the above cache
    // saved `Segments.data` to and the `Seg` is restored
    restore = true;
    graceful_shutdown = false;
    let mut new_cache = make_cache(restore, datapool_path, metadata_path, graceful_shutdown);

    assert!(new_cache.restored());
    // "latte" should be in restored cache
    assert!(new_cache.get(b"latte").is_some());
    assert_eq!(new_cache.items(), 1);
    assert_eq!(new_cache.segments.free(), SEGMENTS - 1);

    // the restored cache should be equivalent to the old cache
    assert!(new_cache == old_cache);
}

// Creates a new cache, gracefully shutsdown cache and restore cache. Check
// caches are equivalent
#[test]
fn new_file_backed_cache_not_changed_and_restored() {
    // Create a temporary directory
    let dir = tmp_dir();
    // Create tempfile for datapool
    let datapool_path: Option<PathBuf> = Some(dir.path().join("datapool"));
    // Create tempfile for `HashTable`
    let metadata_path: Option<PathBuf> = Some(dir.path().join("hashtable"));

    // create new, file backed cache
    let mut restore = false;
    let mut graceful_shutdown = true;

    let cache = make_cache(restore, datapool_path, metadata_path, graceful_shutdown);

    assert!(!cache.restored());

    // Get a copy of the cache to be compared later
    let old_cache = cache.clone();

    // Flush cache
    assert!(cache.flush().is_ok());

    // Create same tempfiles (they have been moved since first created)
    let datapool_path: Option<PathBuf> = Some(dir.path().join("datapool"));
    let metadata_path: Option<PathBuf> = Some(dir.path().join("hashtable"));

    // restore cache
    // This cache is file backed by same file as the above cache
    // saved `Segments.data` to and the `Seg` is restored
    restore = true;
    graceful_shutdown = false;
    let new_cache = make_cache(restore, datapool_path, metadata_path, graceful_shutdown);

    assert!(new_cache.restored());

    // the restored cache should be equivalent to the old cache
    assert!(new_cache == old_cache);
}

// Creates a new cache, stores an item, gracefully shutsdown cache and spawn new
// cache. Check item is not in new cache and caches are not equivalent
#[test]
fn new_cache_changed_and_not_restored() {
    // Create a temporary directory
    let dir = tmp_dir();
    // Create tempfile for datapool
    let datapool_path: Option<PathBuf> = Some(dir.path().join("datapool"));
    // Create tempfile for `HashTable`
    let metadata_path: Option<PathBuf> = Some(dir.path().join("hashtable"));

    // create new, file backed cache
    let mut restore = false;
    let graceful_shutdown = true;
    let mut cache = make_cache(restore, datapool_path, metadata_path, graceful_shutdown);

    assert!(!cache.restored());
    assert_eq!(cache.items(), 0);
    assert_eq!(cache.segments.free(), SEGMENTS);

    // "latte" should not be in a new, empty cache
    assert!(cache.get(b"latte").is_none());
    // insert "latte" into cache
    assert!(cache
        .insert(b"latte", b"", None, Duration::from_secs(5))
        .is_ok());
    // "latte" should now be in cache
    assert!(cache.get(b"latte").is_some());

    assert_eq!(cache.items(), 1);
    assert_eq!(cache.segments.free(), SEGMENTS - 1);

    // Get a copy of the cache to be compared later
    let old_cache = cache.clone();

    // Flush cache
    assert!(cache.flush().is_ok());

    // Create same tempfile (it has been moved since first created)
    let datapool_path: Option<PathBuf> = Some(dir.path().join("datapool"));

    // create new, file backed cache.
    // This new cache is file backed by same file as the above cache
    // saved `Segments.data` to but this cache is treated as new
    restore = false;
    let mut new_cache = make_cache(restore, datapool_path, None, graceful_shutdown);

    assert!(!new_cache.restored());
    assert_eq!(new_cache.items(), 0);
    assert_eq!(new_cache.segments.free(), SEGMENTS);

    // "latte" should not be in new cache
    assert!(new_cache.get(b"latte").is_none());

    // the restored cache should not be equivalent to the old cache
    assert!(new_cache != old_cache);
}

// Create a new cache, fill it with items. Gracefully shutdown this cache.
// Restore cache and check that every key from the original cache exists in the
// restored cache. Check caches are equivalent
#[test]
fn full_cache_recovery_long() {
    // Create a temporary directory
    let dir = tmp_dir();
    // Create tempfile for datapool
    let datapool_path: Option<PathBuf> = Some(dir.path().join("datapool"));
    // Create tempfile for `HashTable`
    let metadata_path: Option<PathBuf> = Some(dir.path().join("hashtable"));

    let ttl = Duration::ZERO;
    let value_size = 512;
    let key_size = 1;
    let iters = 1_000_000;

    // create new, file backed cache
    let mut restore = false;
    let mut graceful_shutdown = true;
    let mut cache = make_cache(restore, datapool_path, metadata_path, graceful_shutdown);

    assert!(!cache.restored());
    assert_eq!(cache.items(), 0);
    assert_eq!(cache.segments.free(), SEGMENTS);

    let mut rng = rand::rng();

    let mut key = vec![0; key_size];
    let mut value = vec![0; value_size];

    // record all of the unique keys
    let mut unique_keys = HashSet::new();

    // fill cache
    for _ in 0..iters {
        rng.fill_bytes(&mut key);
        rng.fill_bytes(&mut value);

        let save_key = key.clone();
        unique_keys.insert(save_key);

        assert!(cache.insert(&key, &value, None, ttl).is_ok());
    }

    // record all active keys in cache
    // (this could be less than # unique keys if eviction has occurred)
    let mut unique_active_keys = Vec::new();
    for key in &unique_keys {
        // if this key exists, save it!
        if cache.get(&key).is_some() {
            unique_active_keys.push(key);
        }
    }

    // check that the number of active items in the cache equals the number
    // of active keys
    assert_eq!(cache.items(), unique_active_keys.len());

    // Get a copy of the cache to be compared later
    let old_cache = cache.clone();

    // Flush cache
    assert!(cache.flush().is_ok());

    // Create same tempfiles (they have been moved since first created)
    let datapool_path: Option<PathBuf> = Some(dir.path().join("datapool"));
    let metadata_path: Option<PathBuf> = Some(dir.path().join("hashtable"));

    // restore cache
    // This new cache is file backed by same file as the above cache
    // saved `Segments.data` to and the `Seg` is restored
    restore = true;
    graceful_shutdown = false;
    let mut new_cache = make_cache(restore, datapool_path, metadata_path, graceful_shutdown);

    assert!(new_cache.restored());

    // the restored cache should be equivalent to the old cache
    assert!(new_cache == old_cache);

    // check that the number of active items in the restored cache
    // equals the number of active keys in the original cache
    assert_eq!(new_cache.items(), unique_active_keys.len());

    // check that every active key from the original cache is in
    // the restored cache
    while let Some(key) = unique_active_keys.pop() {
        assert!(new_cache.get(&key).is_some());
    }
}
