// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Implements the wire protocol for the `Memcache` protocol implementation.

mod request;
mod response;

pub use request::*;
pub use response::*;

use super::*;

use common::metrics::metric;
use metrics::{Counter, Heatmap, Relaxed};

#[metric(name="get", crate=common::metrics)]
static GET: Counter = Counter::new();
#[metric(name="get_ex", crate=common::metrics)]
static GET_EX: Counter = Counter::new();
#[metric(name="get_cardinality", crate=common::metrics)]
static GET_CARDINALITY: Relaxed<Heatmap> = Relaxed::new(|| {
    Heatmap::new(
        request::MAX_BATCH_SIZE as _,
        3,
        PreciseDuration::from_secs(60),
        PreciseDuration::from_secs(1),
    )
});
#[metric(name="get_key", crate=common::metrics)]
static GET_KEY: Counter = Counter::new();
#[metric(name="get_key_hit", crate=common::metrics)]
static GET_KEY_HIT: Counter = Counter::new();
#[metric(name="get_key_miss", crate=common::metrics)]
static GET_KEY_MISS: Counter = Counter::new();

#[metric(name="gets", crate=common::metrics)]
static GETS: Counter = Counter::new();
#[metric(name="gets_ex", crate=common::metrics)]
static GETS_EX: Counter = Counter::new();
#[metric(name="gets_key", crate=common::metrics)]
static GETS_KEY: Counter = Counter::new();
#[metric(name="gets_key_hit", crate=common::metrics)]
static GETS_KEY_HIT: Counter = Counter::new();
#[metric(name="gets_key_miss", crate=common::metrics)]
static GETS_KEY_MISS: Counter = Counter::new();

#[metric(name="set", crate=common::metrics)]
static SET: Counter = Counter::new();
#[metric(name="set_ex", crate=common::metrics)]
static SET_EX: Counter = Counter::new();
#[metric(name="set_stored", crate=common::metrics)]
static SET_STORED: Counter = Counter::new();
#[metric(name="set_not_stored", crate=common::metrics)]
static SET_NOT_STORED: Counter = Counter::new();

#[metric(name="add", crate=common::metrics)]
static ADD: Counter = Counter::new();
#[metric(name="add_ex", crate=common::metrics)]
static ADD_EX: Counter = Counter::new();
#[metric(name="add_stored", crate=common::metrics)]
static ADD_STORED: Counter = Counter::new();
#[metric(name="add_not_stored", crate=common::metrics)]
static ADD_NOT_STORED: Counter = Counter::new();

#[metric(name="replace", crate=common::metrics)]
static REPLACE: Counter = Counter::new();
#[metric(name="replace_ex", crate=common::metrics)]
static REPLACE_EX: Counter = Counter::new();
#[metric(name="replace_stored", crate=common::metrics)]
static REPLACE_STORED: Counter = Counter::new();
#[metric(name="replace_not_stored", crate=common::metrics)]
static REPLACE_NOT_STORED: Counter = Counter::new();

#[metric(name="append", crate=common::metrics)]
static APPEND: Counter = Counter::new();
#[metric(name="append_ex", crate=common::metrics)]
static APPEND_EX: Counter = Counter::new();
#[metric(name="append_stored", crate=common::metrics)]
static APPEND_STORED: Counter = Counter::new();
#[metric(name="append_not_stored", crate=common::metrics)]
static APPEND_NOT_STORED: Counter = Counter::new();

#[metric(name="prepend", crate=common::metrics)]
static PREPEND: Counter = Counter::new();
#[metric(name="prepend_ex", crate=common::metrics)]
static PREPEND_EX: Counter = Counter::new();
#[metric(name="prepend_stored", crate=common::metrics)]
static PREPEND_STORED: Counter = Counter::new();
#[metric(name="prepend_not_stored", crate=common::metrics)]
static PREPEND_NOT_STORED: Counter = Counter::new();

#[metric(name="delete", crate=common::metrics)]
static DELETE: Counter = Counter::new();
#[metric(name="delete_ex", crate=common::metrics)]
static DELETE_EX: Counter = Counter::new();
#[metric(name="delete_deleted", crate=common::metrics)]
static DELETE_DELETED: Counter = Counter::new();
#[metric(name="delete_not_found", crate=common::metrics)]
static DELETE_NOT_FOUND: Counter = Counter::new();

#[metric(name="incr", crate=common::metrics)]
static INCR: Counter = Counter::new();
#[metric(name="incr_ex", crate=common::metrics)]
static INCR_EX: Counter = Counter::new();
#[metric(name="incr_not_found", crate=common::metrics)]
static INCR_NOT_FOUND: Counter = Counter::new();

#[metric(name="decr", crate=common::metrics)]
static DECR: Counter = Counter::new();
#[metric(name="decr_ex", crate=common::metrics)]
static DECR_EX: Counter = Counter::new();
#[metric(name="decr_not_found", crate=common::metrics)]
static DECR_NOT_FOUND: Counter = Counter::new();

#[metric(name="cas", crate=common::metrics)]
static CAS: Counter = Counter::new();
#[metric(name="cas_ex", crate=common::metrics)]
static CAS_EX: Counter = Counter::new();
#[metric(name="cas_exists", crate=common::metrics)]
static CAS_EXISTS: Counter = Counter::new();
#[metric(name="cas_not_found", crate=common::metrics)]
static CAS_NOT_FOUND: Counter = Counter::new();
#[metric(name="cas_stored", crate=common::metrics)]
static CAS_STORED: Counter = Counter::new();
