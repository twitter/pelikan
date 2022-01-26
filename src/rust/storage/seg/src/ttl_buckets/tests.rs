// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use super::*;
use crate::*;

#[test]
fn bucket_index() {
    let ttl_buckets = TtlBuckets::new();

    // Zero TTL and max duration both go into the same TtlBucket
    assert_eq!(ttl_buckets.get_bucket_index(Duration::from_secs(0)), 1023);
    assert_eq!(
        ttl_buckets.get_bucket_index(Duration::from_secs(u32::MAX)),
        1023
    );

    // first bucket is only 7s wide because 0 is no ttl
    assert_eq!(ttl_buckets.get_bucket_index(Duration::from_secs(1)), 0);
    assert_eq!(ttl_buckets.get_bucket_index(Duration::from_secs(7)), 0);

    // buckets from 8s - 2048s (0..34 minutes) are all 8s wide
    for bucket in 1..256 {
        let start = Duration::from_secs(8 * bucket);
        let end = Duration::from_secs(8 * bucket + 7);
        assert_eq!(
            ttl_buckets.get_bucket_index(start) as u32,
            bucket,
            "ttl: {:?}",
            start
        );
        assert_eq!(
            ttl_buckets.get_bucket_index(end) as u32,
            bucket,
            "ttl: {:?}",
            end
        );
    }

    // buckets from 2048s - 32_768s (34 minutes .. 9 hours) are all 128s wide (2 minutes)
    for bucket in 16..256 {
        let start = Duration::from_secs(128 * bucket);
        let end = Duration::from_secs(128 * bucket + 127);
        assert_eq!(
            ttl_buckets.get_bucket_index(start) as u32,
            bucket + 256,
            "ttl: {:?}",
            start
        );
        assert_eq!(
            ttl_buckets.get_bucket_index(end) as u32,
            bucket + 256,
            "ttl: {:?}",
            end
        );
    }

    // buckets from 32_768s - 524_288s (9 hours .. 6 days) are all 2048s wide (34 minutes)
    for bucket in 16..256 {
        let start = Duration::from_secs(2048 * bucket);
        let end = Duration::from_secs(2048 * bucket + 2047);
        assert_eq!(
            ttl_buckets.get_bucket_index(start) as u32,
            bucket + 512,
            "ttl: {:?}",
            start
        );
        assert_eq!(
            ttl_buckets.get_bucket_index(end) as u32,
            bucket + 512,
            "ttl: {:?}",
            end
        );
    }

    // buckets from 524_288s - 8_388_608s (6 days .. 97 days) are all 32_768s wide (9 hours)
    for bucket in 16..256 {
        let start = Duration::from_secs(32_768 * bucket);
        let end = Duration::from_secs(32_768 * bucket + 32_767);
        assert_eq!(
            ttl_buckets.get_bucket_index(start) as u32,
            bucket + 768,
            "ttl: {:?}",
            start
        );
        assert_eq!(
            ttl_buckets.get_bucket_index(end) as u32,
            bucket + 768,
            "ttl: {:?}",
            end
        );
    }

    // TTLs longer than 97 days are the max TTL
    assert_eq!(
        ttl_buckets.get_bucket_index(Duration::from_secs(8_388_608)) as u32,
        1023
    );
}
