// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

mod request;
mod response;
// mod result;
// mod storage;
mod util;

use rustcommon_metrics::{Counter, Heatmap, Relaxed, metric};
pub(crate) use util::*;

pub use request::*;
pub use response::*;
// pub use result::*;
// pub use storage::*;

use common::expiry::TimeType;

pub enum MemcacheError {
    Error(Error),
    ClientError(ClientError),
    ServerError(ServerError),
}

// use rustcommon_metrics::*;

type Instant = common::time::Instant<common::time::Nanoseconds<u64>>;

#[rustfmt::skip]
macro_rules! counter {
    ($identifier:ident, $name:tt) => {
        #[metric(
            name = $name
        )]
        pub static $identifier: Counter = Counter::new();
    };
    ($identifier:ident, $name:tt, $description:tt) => {
        #[metric(
            name = $name,
            description = $description
        )]
        pub static $identifier: Counter = Counter::new();
    };
}

#[rustfmt::skip]
macro_rules! heatmap {
    ($identifier:ident, $name:tt, $max:expr) => {
        #[metric(
            name = $name
        )]
        pub static $identifier: Relaxed<Heatmap> = Relaxed::new(|| {
            Heatmap::new(
                $max as _,
                3,
                rustcommon_metrics::Duration::<rustcommon_metrics::Nanoseconds<u64>>::from_secs(60),
                rustcommon_metrics::Duration::<rustcommon_metrics::Nanoseconds<u64>>::from_secs(1),
            )
        });
    };
    ($identifier:ident, $name:tt, $max:expr, $description:tt) => {
        #[metric(
            name = $name,
            description = $description
        )]
        pub static $identifier: Relaxed<Heatmap> = Relaxed::new(|| {
            Heatmap::new(
                $max as _,
                3,
                rustcommon_metrics::Duration::<rustcommon_metrics::Nanoseconds<u64>>::from_secs(60),
                rustcommon_metrics::Duration::<rustcommon_metrics::Nanoseconds<u64>>::from_secs(1),
            )
        });
    };
}


counter!(COMPOSE_GET, "compose/get");
counter!(COMPOSE_GET_KEY, "compose/get/key");
heatmap!(COMPOSE_GET_CARDINALITY, "compose/get/cardinality", 1_000_000);
counter!(COMPOSE_GETS, "compose/gets");
counter!(COMPOSE_GETS_KEY, "compose/gets/key");
heatmap!(COMPOSE_GETS_CARDINALITY, "compose/gets/cardinality", 1_000_000);
counter!(COMPOSE_SET, "compose/set");
counter!(COMPOSE_ADD, "compose/add");
counter!(COMPOSE_REPLACE, "compose/replace");
counter!(COMPOSE_CAS, "compose/cas");
counter!(COMPOSE_INCR, "compose/incr");
counter!(COMPOSE_DECR, "compose/decr");
counter!(COMPOSE_APPEND, "compose/append");
counter!(COMPOSE_PREPEND, "compose/prepend");
counter!(COMPOSE_DELETE, "compose/delete");
counter!(COMPOSE_QUIT, "compose/quit");
counter!(COMPOSE_FLUSH_ALL, "compose/flush_all");

counter!(PARSE_GET, "parse/get");
counter!(PARSE_GET_KEY, "parse/get/key");
heatmap!(PARSE_GET_CARDINALITY, "parse/get/cardinality", 1_000_000);
counter!(PARSE_GET_EX, "parse/get_ex");
counter!(PARSE_GETS, "parse/gets");
counter!(PARSE_GETS_KEY, "parse/gets/key");
heatmap!(PARSE_GETS_CARDINALITY, "parse/gets/cardinality", 1_000_000);
counter!(PARSE_GETS_EX, "parse/gets_ex");
counter!(PARSE_SET, "parse/set");
counter!(PARSE_SET_EX, "parse/set_ex");
counter!(PARSE_ADD, "parse/add");
counter!(PARSE_ADD_EX, "parse/add_ex");
counter!(PARSE_REPLACE, "parse/replace");
counter!(PARSE_REPLACE_EX, "parse/replace_ex");
counter!(PARSE_CAS, "parse/cas");
counter!(PARSE_CAS_EX, "parse/cas_ex");
counter!(PARSE_INCR, "parse/incr");
counter!(PARSE_INCR_EX, "parse/incr_ex");
counter!(PARSE_DECR, "parse/decr");
counter!(PARSE_DECR_EX, "parse/decr_ex");
counter!(PARSE_APPEND, "parse/append");
counter!(PARSE_APPEND_EX, "parse/append_ex");
counter!(PARSE_PREPEND, "parse/prepend");
counter!(PARSE_PREPEND_EX, "parse/prepend_ex");
counter!(PARSE_DELETE, "parse/delete");
counter!(PARSE_DELETE_EX, "parse/delete_ex");
counter!(PARSE_QUIT, "parse/quit");
counter!(PARSE_FLUSH_ALL, "parse/flush_all");
counter!(PARSE_FLUSH_ALL_EX, "parse/flush_all_ex");
