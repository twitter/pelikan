// ccommon - a cache common library.
// Copyright (C) 2019 Twitter, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Types and methods for dealing with ccommon metrics.
//!
//! To use this any collection of metrics (i.e. a struct containing metrics)
//! should implement the `Metrics` trait through the derive macro. This
//! will (usually) assert that the struct you are using is equivalent in
//! memory to an array of `metric` structs.

use std::ffi::CStr;

use cc_binding::{metric, metric_describe_all, metric_reset};

// Sealed trait to prevent SingleMetric from ever being implemented
// from outside of this crate.
mod private {
    pub trait Sealed {}
}

mod counter;
mod fpn;
mod gauge;

pub use self::counter::Counter;
pub use self::fpn::Fpn;
pub use self::gauge::Gauge;

/// A single metric value.
///
/// This trait is sealed and cannot be implemented outside
/// of ccommon_rs.
pub unsafe trait SingleMetric: self::private::Sealed {
    /// Create a metric with the given description and name.
    ///
    /// Normally this should only be called by the
    /// derive macro for `Metrics`.
    fn new(name: &'static CStr, desc: &'static CStr) -> Self;

    /// The metric's name
    fn name(&self) -> &'static CStr;
    /// The metric's description
    fn desc(&self) -> &'static CStr;
}

/// A type that can be safely viewed as a contiguous array
/// of [`metric`s][0].
///
/// It should usually only be implemented through `#[derive(Metrics)]`.
/// However, it must be implemented manually for C types that have
/// been bound through bindgen.
///
/// [0]: ../../cc_binding/struct.metric.html
pub unsafe trait Metrics: Sized {
    fn new() -> Self;
}

pub trait MetricExt: Metrics {
    /// The number of metrics in this object when
    /// it is interpreted as an array.
    ///
    /// # Panics
    /// Panics if the size of this type is not a multiple
    /// of the size of `metric`.
    fn num_metrics() -> usize {
        use std::mem::size_of;

        // If this assert fails then there was no way that
        // options upholds it's safety requirements so it's
        // better to fail here.
        assert!(size_of::<Self>() % size_of::<metric>() == 0);

        // If this assert fails then we'll pass an invalid
        // size to several ccommon methods.
        assert!(size_of::<Self>() / size_of::<metric>() < std::u32::MAX as usize);

        size_of::<Self>() / size_of::<metric>()
    }

    /// Get `self` as a const pointer to an array of `metric`s.
    ///
    /// # Panics
    /// Panics if the size of this type is not a multiple
    /// of the size of `metric`.
    fn as_ptr(&self) -> *const metric {
        assert!(std::mem::size_of::<Self>() % std::mem::size_of::<metric>() == 0);

        self as *const _ as *const metric
    }

    /// Get `self` as a mutable pointer to an array of `metric`s.
    ///
    /// # Panics
    /// Panics if the size of this type is not a multiple
    /// of the size of `metric`.
    fn as_mut_ptr(&mut self) -> *mut metric {
        assert!(std::mem::size_of::<Self>() % std::mem::size_of::<metric>() == 0);

        self as *mut _ as *mut metric
    }

    /// Print a description of all metricss in the current object
    /// given using the name and description.
    ///
    /// Internally this calls out to `metric_describe_all`.
    fn describe_all(&self) {
        unsafe {
            metric_describe_all(
                // Note: ccommon uses a mutable pointer but it
                //       should really be a const pointer.
                self.as_ptr() as *mut _,
                Self::num_metrics() as u32,
            )
        }
    }

    /// Reset all metrics to their default values.
    ///
    /// This means that all 3 types of metrics are reset to 0.
    ///
    /// Internally this calls out to `metric_reset`.
    fn reset_all(&mut self) {
        unsafe { metric_reset(self.as_mut_ptr(), Self::num_metrics() as u32) }
    }
}

impl<T: Metrics> MetricExt for T {}

/// Impls of Metrics for cc_bindings types
mod impls {
    use super::Metrics;
    use cc_binding::*;

    macro_rules! c_str {
        ($s:expr) => {
            concat!($s, "\0").as_bytes().as_ptr() as *const i8 as *mut _
        };
    }

    macro_rules! initialize_metric_value {
        (METRIC_GAUGE) => {
            metric_anon_union::gauge(0)
        };
        (METRIC_COUNTER) => {
            metric_anon_union::counter(0)
        };
        (METRIC_FPN) => {
            metric_anon_union::fpn(0.0)
        };
    }

    macro_rules! impl_metrics {
        {
            $(
                impl Metrics for $metrics_ty:ty {
                    $(
                        ACTION( $field:ident, $type:ident, $desc:expr )
                    )*
                }
            )*
        } => {
            $(
                unsafe impl Metrics for $metrics_ty {
                    fn new() -> Self {
                        Self {
                            $(
                                $field: metric {
                                    name: c_str!(stringify!($field)),
                                    type_: $type,
                                    desc: c_str!($desc),
                                    data: initialize_metric_value!($type)
                                },
                            )*
                        }
                    }
                }
            )*
        }
    }

    impl_metrics! {
        impl Metrics for buf_metrics_st {
            ACTION( buf_curr,         METRIC_GAUGE,   "# buf allocated"                        )
            ACTION( buf_active,       METRIC_GAUGE,   "# buf in use/borrowed"                  )
            ACTION( buf_create,       METRIC_COUNTER, "# buf creates"                          )
            ACTION( buf_create_ex,    METRIC_COUNTER, "# buf create exceptions"                )
            ACTION( buf_destroy,      METRIC_COUNTER, "# buf destroys"                         )
            ACTION( buf_borrow,       METRIC_COUNTER, "# buf borrows"                          )
            ACTION( buf_borrow_ex,    METRIC_COUNTER, "# buf borrow exceptions"                )
            ACTION( buf_return,       METRIC_COUNTER, "# buf returns"                          )
            ACTION( buf_memory,       METRIC_GAUGE,   "memory alloc'd to buf including header" )
        }

        impl Metrics for dbuf_metrics_st {
            ACTION( dbuf_double,    METRIC_COUNTER, "# double completed"   )
            ACTION( dbuf_double_ex, METRIC_COUNTER, "# double failed"      )
            ACTION( dbuf_shrink,    METRIC_COUNTER, "# shrink completed"   )
            ACTION( dbuf_shrink_ex, METRIC_COUNTER, "# shrink failed"      )
            ACTION( dbuf_fit,       METRIC_COUNTER, "# fit completed"      )
            ACTION( dbuf_fit_ex,    METRIC_COUNTER, "# fit failed"         )
        }

        impl Metrics for pipe_metrics_st {
            ACTION( pipe_conn_create,    METRIC_COUNTER, "# pipe connections created"    )
            ACTION( pipe_conn_create_ex, METRIC_COUNTER, "# pipe conn create exceptions" )
            ACTION( pipe_conn_destroy,   METRIC_COUNTER, "# pipe connections destroyed"  )
            ACTION( pipe_conn_curr ,     METRIC_GAUGE,   "# pipe conn allocated"         )
            ACTION( pipe_conn_borrow,    METRIC_COUNTER, "# pipe connections borrowed"   )
            ACTION( pipe_conn_borrow_ex, METRIC_COUNTER, "# pipe conn borrow exceptions" )
            ACTION( pipe_conn_return,    METRIC_COUNTER, "# pipe connections returned"   )
            ACTION( pipe_conn_active,    METRIC_GAUGE,   "# pipe conn being borrowed"    )
            ACTION( pipe_open,           METRIC_COUNTER, "# pipe connects made"          )
            ACTION( pipe_open_ex,        METRIC_COUNTER, "# pipe connect exceptions"     )
            ACTION( pipe_close,          METRIC_COUNTER, "# pipe connection closed"      )
            ACTION( pipe_recv,           METRIC_COUNTER, "# recv attempted"              )
            ACTION( pipe_recv_ex,        METRIC_COUNTER, "# recv exceptions"             )
            ACTION( pipe_recv_byte,      METRIC_COUNTER, "# bytes received"              )
            ACTION( pipe_send,           METRIC_COUNTER, "# send attempted"              )
            ACTION( pipe_send_ex,        METRIC_COUNTER, "# send exceptions"             )
            ACTION( pipe_send_byte,      METRIC_COUNTER, "# bytes sent"                  )
            ACTION( pipe_flag_ex,        METRIC_COUNTER, "# pipe flag exceptions"        )
        }

        impl Metrics for tcp_metrics_st {
            ACTION( tcp_conn_create,    METRIC_COUNTER, "# tcp connections created"    )
            ACTION( tcp_conn_create_ex, METRIC_COUNTER, "# tcp conn create exceptions" )
            ACTION( tcp_conn_destroy,   METRIC_COUNTER, "# tcp connections destroyed"  )
            ACTION( tcp_conn_curr,      METRIC_GAUGE,   "# tcp conn allocated"         )
            ACTION( tcp_conn_borrow,    METRIC_COUNTER, "# tcp connections borrowed"   )
            ACTION( tcp_conn_borrow_ex, METRIC_COUNTER, "# tcp conn borrow exceptions" )
            ACTION( tcp_conn_return,    METRIC_COUNTER, "# tcp connections returned"   )
            ACTION( tcp_conn_active,    METRIC_GAUGE,   "# tcp conn being borrowed"    )
            ACTION( tcp_accept,         METRIC_COUNTER, "# tcp connection accepts"     )
            ACTION( tcp_accept_ex,      METRIC_COUNTER, "# tcp accept exceptions"      )
            ACTION( tcp_reject,         METRIC_COUNTER, "# tcp connection rejects"     )
            ACTION( tcp_reject_ex,      METRIC_COUNTER, "# tcp reject exceptions"      )
            ACTION( tcp_connect,        METRIC_COUNTER, "# tcp connects made"          )
            ACTION( tcp_connect_ex,     METRIC_COUNTER, "# tcp connect exceptions "    )
            ACTION( tcp_close,          METRIC_COUNTER, "# tcp connection closed"      )
            ACTION( tcp_recv,           METRIC_COUNTER, "# recv attempted"             )
            ACTION( tcp_recv_ex,        METRIC_COUNTER, "# recv exceptions"            )
            ACTION( tcp_recv_byte,      METRIC_COUNTER, "# bytes received"             )
            ACTION( tcp_send,           METRIC_COUNTER, "# send attempted"             )
            ACTION( tcp_send_ex,        METRIC_COUNTER, "# send exceptions"            )
            ACTION( tcp_send_byte,      METRIC_COUNTER, "# bytes sent"                 )
        }

        impl Metrics for sockio_metrics_st {
            ACTION( buf_sock_create,    METRIC_COUNTER, "# buf sock created"           )
            ACTION( buf_sock_create_ex, METRIC_COUNTER, "# buf sock create exceptions" )
            ACTION( buf_sock_destroy,   METRIC_COUNTER, "# buf sock destroyed"         )
            ACTION( buf_sock_curr,      METRIC_GAUGE,   "# buf sock allocated"         )
            ACTION( buf_sock_borrow,    METRIC_COUNTER, "# buf sock borrowed"          )
            ACTION( buf_sock_borrow_ex, METRIC_COUNTER, "# buf sock borrow exceptions" )
            ACTION( buf_sock_return,    METRIC_COUNTER, "# buf sock returned"          )
            ACTION( buf_sock_active,    METRIC_GAUGE,   "# buf sock being borrowed"    )
        }

        impl Metrics for timing_wheel_metrics_st {
            ACTION( timeout_event_curr,     METRIC_GAUGE,   "# timeout events allocated"   )
            ACTION( timeout_event_active,   METRIC_GAUGE,   "# timeout events in use"      )
            ACTION( timeout_event_borrow,   METRIC_COUNTER, "# timeout events borrowed"    )
            ACTION( timeout_event_borrow_ex,METRIC_COUNTER, "# tevents borrow errors"      )
            ACTION( timeout_event_return,   METRIC_COUNTER, "# timeout events returned"    )
            ACTION( timing_wheel_insert,    METRIC_COUNTER, "# tevent insertions"          )
            ACTION( timing_wheel_remove,    METRIC_COUNTER, "# tevent removal"             )
            ACTION( timing_wheel_event,     METRIC_GAUGE,   "# tevents in timing wheels"   )
            ACTION( timing_wheel_process,   METRIC_COUNTER, "# tevents processed"          )
            ACTION( timing_wheel_tick,      METRIC_COUNTER, "# ticks processed"            )
            ACTION( timing_wheel_exec,      METRIC_COUNTER, "# timing wheel executions "   )
        }

        impl Metrics for event_metrics_st {
            ACTION( event_total,        METRIC_COUNTER, "# events returned"    )
            ACTION( event_loop,         METRIC_COUNTER, "# event loop returns" )
            ACTION( event_read,         METRIC_COUNTER, "# reads registered"   )
            ACTION( event_write,        METRIC_COUNTER, "# writes registered"  )
        }

        impl Metrics for log_metrics_st {
            ACTION( log_create,     METRIC_COUNTER, "# loggers created"                )
            ACTION( log_create_ex,  METRIC_COUNTER, "# log create errors"              )
            ACTION( log_destroy,    METRIC_COUNTER, "# loggers destroyed"              )
            ACTION( log_curr,       METRIC_GAUGE,   "current # loggers"                )
            ACTION( log_open,       METRIC_COUNTER, "# files opened by loggers"        )
            ACTION( log_open_ex,    METRIC_COUNTER, "# logger open file errors"        )
            ACTION( log_write,      METRIC_COUNTER, "# log messages written"           )
            ACTION( log_write_byte, METRIC_COUNTER, "# bytes written by log"           )
            ACTION( log_write_ex,   METRIC_COUNTER, "# log write errors"               )
            ACTION( log_skip,       METRIC_COUNTER, "# messages not completely logged" )
            ACTION( log_skip_byte,  METRIC_COUNTER, "# bytes unable to be logged"      )
            ACTION( log_flush,      METRIC_COUNTER, "# log flushes to disk"            )
            ACTION( log_flush_ex,   METRIC_COUNTER, "# errors flushing to disk"        )
        }

        impl Metrics for rbuf_metrics_st {
            ACTION( rbuf_create,    METRIC_COUNTER, "# rbuf created"         )
            ACTION( rbuf_create_ex, METRIC_COUNTER, "# rbuf create errors"   )
            ACTION( rbuf_destroy,   METRIC_COUNTER, "# rbuf destroyed"       )
            ACTION( rbuf_curr,      METRIC_GAUGE,   "# rbuf allocated"       )
            ACTION( rbuf_byte,      METRIC_GAUGE,   "# rbuf bytes allocated" )
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    macro_rules! c_str {
        ($s:expr) => {
            unsafe { CStr::from_bytes_with_nul_unchecked(concat!($s, "\0").as_bytes()) }
        };
    }

    #[test]
    fn gauge_basic_use() {
        let gauge = Gauge::new(c_str!("test"), c_str!("test desc"));

        assert_eq!(gauge.value(), 0);
        gauge.incr();
        assert_eq!(gauge.value(), 1);
        gauge.decr_n(10);
        assert_eq!(gauge.value(), -9);
    }

    #[test]
    fn counter_basic_use() {
        let ctr = Counter::new(c_str!("test"), c_str!("test desc"));

        assert_eq!(ctr.value(), 0);
        ctr.incr();
        assert_eq!(ctr.value(), 1);
        ctr.incr_n(400);
        assert_eq!(ctr.value(), 401);
    }

    #[test]
    fn fpn_basic_use() {
        let fpn = Fpn::new(c_str!("test"), c_str!("test desc"));

        assert_eq!(fpn.value(), 0.0);
        fpn.update(500.0);
        assert_eq!(fpn.value(), 500.0);
    }

    #[test]
    fn size_sanity_test() {
        // Protect against a bad bindgen run
        assert!(std::mem::size_of::<metric>() != 0);
    }
}
