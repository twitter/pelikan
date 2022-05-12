// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Implements the wire protocol for the `Memcache` protocol implementation.

mod request;
mod response;

pub use request::*;
pub use response::*;

use super::*;

use common::metrics::{static_metrics, Counter, Heatmap, Relaxed};
use common::{counter, heatmap};

counter!(GET, "total number of get requests");
heatmap!(
    GET_CARDINALITY,
    request::MAX_BATCH_SIZE,
    "distribution of key cardinality for get requests"
);
counter!(GET_EX, "number of get requests resulting in an exception");
counter!(GET_KEY, "total number of keys fetched in get requests");
counter!(
    GET_KEY_HIT,
    "number of keys fetched in get requests that resulted in a cache hit"
);
counter!(
    GET_KEY_MISS,
    "number of keys fetched in get requests that resulted in a cache miss"
);

static_metrics! {
    static GETS: Counter;
    static GETS_EX: Counter;
    static GETS_KEY: Counter;
    static GETS_KEY_HIT: Counter;
    static GETS_KEY_MISS: Counter;

    static SET: Counter;
    static SET_EX: Counter;
    static SET_STORED: Counter;
    static SET_NOT_STORED: Counter;

    static ADD: Counter;
    static ADD_EX: Counter;
    static ADD_STORED: Counter;
    static ADD_NOT_STORED: Counter;

    static REPLACE: Counter;
    static REPLACE_EX: Counter;
    static REPLACE_STORED: Counter;
    static REPLACE_NOT_STORED: Counter;

    static APPEND: Counter;
    static APPEND_EX: Counter;
    static APPEND_STORED: Counter;
    static APPEND_NOT_STORED: Counter;

    static PREPEND: Counter;
    static PREPEND_EX: Counter;
    static PREPEND_STORED: Counter;
    static PREPEND_NOT_STORED: Counter;

    static DELETE: Counter;
    static DELETE_EX: Counter;
    static DELETE_DELETED: Counter;
    static DELETE_NOT_FOUND: Counter;

    static INCR: Counter;
    static INCR_EX: Counter;
    static INCR_NOT_FOUND: Counter;

    static DECR: Counter;
    static DECR_EX: Counter;
    static DECR_NOT_FOUND: Counter;

    static CAS: Counter;
    static CAS_EX: Counter;
    static CAS_EXISTS: Counter;
    static CAS_NOT_FOUND: Counter;
    static CAS_STORED: Counter;
}
