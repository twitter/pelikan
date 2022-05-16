use crate::PreciseDuration;
use common::metrics::{static_metrics, Counter, Heatmap, Relaxed};
use common::{counter, heatmap};

counter!(GET, "total number of get requests");
heatmap!(
    GET_CARDINALITY,
    super::request::MAX_BATCH_SIZE,
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
    pub static GETS: Counter;
    pub static GETS_EX: Counter;
    pub static GETS_KEY: Counter;
    pub static GETS_KEY_HIT: Counter;
    pub static GETS_KEY_MISS: Counter;
    pub static SET: Counter;
    pub static SET_EX: Counter;
    pub static SET_STORED: Counter;
    pub static SET_NOT_STORED: Counter;
    pub static ADD: Counter;
    pub static ADD_EX: Counter;
    pub static ADD_STORED: Counter;
    pub static ADD_NOT_STORED: Counter;
    pub static REPLACE: Counter;
    pub static REPLACE_EX: Counter;
    pub static REPLACE_STORED: Counter;
    pub static REPLACE_NOT_STORED: Counter;
    pub static APPEND: Counter;
    pub static APPEND_EX: Counter;
    pub static APPEND_STORED: Counter;
    pub static APPEND_NOT_STORED: Counter;
    pub static PREPEND: Counter;
    pub static PREPEND_EX: Counter;
    pub static PREPEND_STORED: Counter;
    pub static PREPEND_NOT_STORED: Counter;
    pub static DELETE: Counter;
    pub static DELETE_EX: Counter;
    pub static DELETE_DELETED: Counter;
    pub static DELETE_NOT_FOUND: Counter;
    pub static INCR: Counter;
    pub static INCR_EX: Counter;
    pub static INCR_NOT_FOUND: Counter;
    pub static DECR: Counter;
    pub static DECR_EX: Counter;
    pub static DECR_NOT_FOUND: Counter;
    pub static CAS: Counter;
    pub static CAS_EX: Counter;
    pub static CAS_EXISTS: Counter;
    pub static CAS_NOT_FOUND: Counter;
    pub static CAS_STORED: Counter;
}