use super::*;
use rustcommon_time::CoarseDuration;

#[test]
fn bucket_index() {
    let ttl_buckets = TtlBuckets::new();
    assert_eq!(ttl_buckets.get_bucket_index(CoarseDuration::ZERO), 1023);
    assert_eq!(ttl_buckets.get_bucket_index(CoarseDuration::MAX), 1023);
}
