// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

pub use rustcommon_metrics::{metric, Counter, Gauge, Heatmap, Relaxed};

#[doc(hidden)]
pub extern crate rustcommon_metrics;
#[doc(hidden)]
pub use macros::to_lowercase;

/// Create a set of metrics. Metrics are named with the lowercase name of the
/// static that they are declared with.
///
/// # Example
/// ```
/// # use metrics::*;
/// static_metrics! {
///     // Creates a counter metric called "my_metric" and constructs it by
///     // calling Counter::new.
///     static MY_METRIC: Counter;
///
///     // Creates a gauge metric called "some_other_metric" and initializes
///     // it to have the value 8.
///     pub static SOME_OTHER_METRIC: Gauge = Gauge::with_value(8);
/// }
/// ```
#[macro_export]
macro_rules! static_metrics {
    {$(
        $( #[ $attr:meta ] )*
        $vis:vis static $name:ident : $ty:ty $( = $init:expr )?;
    )*} => {$(
        #[$crate::metric(
            name = $crate::to_lowercase!($name),
            crate = $crate::rustcommon_metrics
        )]
        $( #[ $attr ] )*
        $vis static $name : $ty = $crate::static_metrics!(
            crate __internal; [ $( $init, )? <$ty>::new() ]
        );
    )*};

    // Used to internally take the first expression
    ( crate __internal; [ $a:expr $( , $rest:expr )* ] ) => { $a };
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

static_metrics! {
    static PID: Gauge;
}

pub fn init() {
    PID.set(std::process::id().into());
}
