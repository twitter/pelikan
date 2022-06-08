use rustcommon_metrics::*;

#[doc(hidden)]
pub use macros::to_lowercase;

/// Creates a test that verifies that no two metrics have the same name.
#[macro_export]
#[rustfmt::skip]
macro_rules! test_no_duplicates {
    () => {
        #[cfg(test)]
        mod __metrics_tests {
            #[test]
            fn assert_no_duplicate_metric_names() {
                use std::collections::HashSet;
                use rustcommon_metrics::*;

                let mut seen = HashSet::new();
                for metric in metrics().static_metrics() {
                    let name = metric.name();
                    assert!(seen.insert(name), "found duplicate metric name '{}'", name);
                }
            }
        }
    };
}

pub use test_no_duplicates;

gauge!(PID, "the process id");

pub fn init() {
    PID.set(std::process::id().into());
}
