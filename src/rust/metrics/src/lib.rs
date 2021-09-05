// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

// pub use rustcommon_fastmetrics::*;

pub use macros::to_lowercase;
pub use rustcommon_metrics::{metric, Counter, Gauge};

#[doc(hidden)]
pub extern crate rustcommon_metrics;

#[macro_export]
macro_rules! pelikan_metrics {
    {$(
        $( #[ $attr:meta ] )*
        $vis:vis static $name:ident : $ty:ty ;
    )*} => {$(
        #[$crate::metric(
            name = $crate::to_lowercase!($name),
            crate = $crate::rustcommon_metrics
        )]
        $( #[ $attr ] )*
        $vis static $name : $ty = <$ty>::new();
    )*};
}

/// Creates a test that verifies that no two metrics have the same name.
#[macro_export]
macro_rules! test_no_duplicates {
    () => {
        #[cfg(test)]
        mod __metrics_tests {
            #[test]
            fn assert_no_duplicate_metric_names() {
                use std::collections::HashSet;
                use $crate::rustcommon_metrics::*;

                let mut seen = HashSet::new();
                for metric in metrics().static_metrics() {
                    let name = metric.name();
                    assert!(seen.insert(name), "found duplicate metric name '{}'", name);
                }
            }
        }
    };
}

pelikan_metrics! {
    static PID: Gauge;
}

pub fn init() {
    PID.set(std::process::id().into());
}
