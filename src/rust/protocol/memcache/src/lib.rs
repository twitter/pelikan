// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

mod request;
mod response;
mod result;
mod storage;
mod util;

pub(crate) use util::*;

pub use request::*;
pub use response::*;
pub use result::*;
pub use storage::*;

use common::expiry::TimeType;

pub enum MemcacheError {
    Error(Error),
    ClientError(ClientError),
    ServerError(ServerError),
}

use rustcommon_metrics::*;

type Instant = common::time::Instant<common::time::Nanoseconds<u64>>;

counter!(GET);
heatmap!(GET_CARDINALITY, 1_000_000, "distribution of key cardinality for get requests");
counter!(GET_EX);
counter!(GET_KEY);
counter!(GET_KEY_HIT);
counter!(GET_KEY_MISS);

counter!(GETS);
counter!(GETS_EX);
counter!(GETS_KEY);
counter!(GETS_KEY_HIT);
counter!(GETS_KEY_MISS);

counter!(SET);
counter!(SET_EX);
counter!(SET_STORED);
counter!(SET_NOT_STORED);

counter!(ADD);
counter!(ADD_EX);
counter!(ADD_STORED);
counter!(ADD_NOT_STORED);

counter!(REPLACE);
counter!(REPLACE_EX);
counter!(REPLACE_STORED);
counter!(REPLACE_NOT_STORED);

counter!(APPEND);
counter!(APPEND_EX);
counter!(APPEND_STORED);
counter!(APPEND_NOT_STORED);

counter!(PREPEND);
counter!(PREPEND_EX);
counter!(PREPEND_STORED);
counter!(PREPEND_NOT_STORED);

counter!(DELETE);
counter!(DELETE_EX);
counter!(DELETE_DELETED);
counter!(DELETE_NOT_FOUND);

counter!(INCR);
counter!(INCR_EX);
counter!(INCR_NOT_FOUND);

counter!(DECR);
counter!(DECR_EX);
counter!(DECR_NOT_FOUND);

counter!(CAS);
counter!(CAS_EX);
counter!(CAS_EXISTS);
counter!(CAS_NOT_FOUND);
counter!(CAS_STORED);

counter!(FLUSH_ALL);
counter!(FLUSH_ALL_EX);

counter!(QUIT);
