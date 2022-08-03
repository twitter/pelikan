// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

pub use protocol_common::*;

mod admin;

pub use admin::*;

pub static PERCENTILES: &[(&str, f64)] = &[
    ("p25", 25.0),
    ("p50", 50.0),
    ("p75", 75.0),
    ("p90", 90.0),
    ("p99", 99.0),
    ("p999", 99.9),
    ("p9999", 99.99),
];
