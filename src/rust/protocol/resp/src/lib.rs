// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

mod request;
mod response;
// mod result;
// mod storage;
mod util;

pub(crate) use util::*;

pub use request::*;
pub use response::*;
// pub use result::*;
// pub use storage::*;

use rustcommon_metrics::{metric, Counter};

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
            description = $description,
        )]
        pub static $identifier: Counter = Counter::new();
    };
}

counter!(COMPOSE_GET, "compose/get");
counter!(COMPOSE_SET, "compose/set");

counter!(PARSE_GET, "parse/get");
counter!(PARSE_GET_EX, "parse/get/exception");
counter!(PARSE_SET, "parse/set");
counter!(PARSE_SET_EX, "parse/set/exception");
