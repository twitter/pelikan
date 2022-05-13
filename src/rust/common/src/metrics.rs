pub use rustcommon_metrics::*;

#[doc(hidden)]
pub use macros::to_lowercase;

#[macro_export]
#[cfg_attr(rustfmt, rustfmt_skip)]
macro_rules! counter {
    ($name:ident) => {
        #[$crate::metrics::metric(
            name = $crate::metrics::to_lowercase!($name),
            crate = $crate::metrics
        )]
        pub static $name: Counter = Counter::new();
    };
    ($name:ident, $description:tt) => {
        #[$crate::metrics::metric(
            name = $crate::metrics::to_lowercase!($name),
            description = $description,
            crate = $crate::metrics
        )]
        pub static $name: Counter = Counter::new();
    };
}

#[macro_export]
#[cfg_attr(rustfmt, rustfmt_skip)]
macro_rules! gauge {
    ($name:ident) => {
        #[$crate::metrics::metric(
            name = $crate::metrics::to_lowercase!($name),
            crate = $crate::metrics
        )]
        pub static $name: Gauge = Gauge::new();
    };
    ($name:ident, $description:tt) => {
        #[$crate::metrics::metric(
            name = $crate::metrics::to_lowercase!($name),
            description = $description,
            crate = $crate::metrics
        )]
        pub static $name: Gauge = Gauge::new();
    };
}

#[macro_export]
#[cfg_attr(rustfmt, rustfmt_skip)]
macro_rules! heatmap {
    ($name:ident, $max:expr) => {
        #[$crate::metrics::metric(
            name = $crate::metrics::to_lowercase!($name),
            crate = $crate::metrics
        )]
        pub static $name: Relaxed<Heatmap> = Relaxed::new(|| {
            Heatmap::new(
                $max as _,
                3,
                PreciseDuration::from_secs(60),
                PreciseDuration::from_secs(1),
            )
        });
    };
    ($name:ident, $max:expr, $description:tt) => {
        #[$crate::metrics::metric(
            name = $crate::metrics::to_lowercase!($name),
            description = $description,
            crate = $crate::metrics
        )]
        pub static $name: Relaxed<Heatmap> = Relaxed::new(|| {
            Heatmap::new(
                $max as _,
                3,
                PreciseDuration::from_secs(60),
                PreciseDuration::from_secs(1),
            )
        });
    };
}

#[macro_export]
#[cfg_attr(rustfmt, rustfmt_skip)]
macro_rules! static_metrics {
        {$(
            $( #[ $attr:meta ] )*
            $vis:vis static $name:ident : $ty:ty $( = $init:expr )?;
        )*} => {$(
            #[$crate::metrics::metric(
                name = $crate::metrics::to_lowercase!($name),
                crate = $crate::metrics
            )]
            $( #[ $attr ] )*
            $vis static $name : $ty = static_metrics!(
                crate __internal; [ $( $init, )? <$ty>::new() ]
            );
        )*};

        // Used to internally take the first expression
        ( crate __internal; [ $a:expr $( , $rest:expr )* ] ) => { $a };
    }

/// Creates a test that verifies that no two metrics have the same name.
#[macro_export]
#[cfg_attr(rustfmt, rustfmt_skip)]
macro_rules! test_no_duplicates {
    () => {
        #[cfg(test)]
        mod __metrics_tests {
            #[test]
            fn assert_no_duplicate_metric_names() {
                use std::collections::HashSet;
                use $crate::metrics::*;

                let mut seen = HashSet::new();
                for metric in metrics().static_metrics() {
                    let name = metric.name();
                    assert!(seen.insert(name), "found duplicate metric name '{}'", name);
                }
            }
        }
    };
}

pub use static_metrics;
pub use test_no_duplicates;

crate::gauge!(PID, "the process id");

pub fn init() {
    PID.set(std::process::id().into());
}
