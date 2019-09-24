use ccommon::util::AsMetricArray;
use ccommon_sys::*;
use pelikan_sys::{
    core::{server_metrics_st, worker_metrics_st},
    init_metric,
    protocol::ping::{compose_rsp_metrics_st, parse_req_metrics_st},
    util::procinfo_metrics_st,
};

#[rustfmt::skip]
#[repr(C)]
pub struct Metrics {
    // Perf info
    pub procinfo:       procinfo_metrics_st,
    // Application Modules
    pub parse_req:      parse_req_metrics_st,
    pub compose_rsp:    compose_rsp_metrics_st,
    pub server:         server_metrics_st,
    pub worker:         worker_metrics_st,
    // Common libraries
    pub buf:            buf_metrics_st,
    pub dbuf:           dbuf_metrics_st,
    pub event:          event_metrics_st,
    pub log:            log_metrics_st,
    pub sockio:         sockio_metrics_st,
    pub tcp:            tcp_metrics_st,
    pub timing_wheel:   timing_wheel_metrics_st
}

impl Metrics {
    pub const fn new() -> Metrics {
        Metrics {
            procinfo: procinfo_metrics_st::new(),
            parse_req: parse_req_metrics_st::new(),
            compose_rsp: compose_rsp_metrics_st::new(),
            server: server_metrics_st::new(),
            worker: worker_metrics_st::new(),

            buf: init_metric! {
                buf_metrics_st {
                    ACTION( buf_curr,         METRIC_GAUGE,   "# buf allocated"                        ),
                    ACTION( buf_active,       METRIC_GAUGE,   "# buf in use/borrowed"                  ),
                    ACTION( buf_create,       METRIC_COUNTER, "# buf creates"                          ),
                    ACTION( buf_create_ex,    METRIC_COUNTER, "# buf create exceptions"                ),
                    ACTION( buf_destroy,      METRIC_COUNTER, "# buf destroys"                         ),
                    ACTION( buf_borrow,       METRIC_COUNTER, "# buf borrows"                          ),
                    ACTION( buf_borrow_ex,    METRIC_COUNTER, "# buf borrow exceptions"                ),
                    ACTION( buf_return,       METRIC_COUNTER, "# buf returns"                          ),
                    ACTION( buf_memory,       METRIC_GAUGE,   "memory alloc'd to buf including header" )
                }
            },
            dbuf: init_metric! {
                dbuf_metrics_st {
                    ACTION( dbuf_double,    METRIC_COUNTER, "# double completed"   ),
                    ACTION( dbuf_double_ex, METRIC_COUNTER, "# double failed"      ),
                    ACTION( dbuf_shrink,    METRIC_COUNTER, "# shrink completed"   ),
                    ACTION( dbuf_shrink_ex, METRIC_COUNTER, "# shrink failed"      ),
                    ACTION( dbuf_fit,       METRIC_COUNTER, "# fit completed"      ),
                    ACTION( dbuf_fit_ex,    METRIC_COUNTER, "# fit failed"         )
                }
            },
            event: init_metric! {
                event_metrics_st {
                    ACTION( event_total,        METRIC_COUNTER, "# events returned"    ),
                    ACTION( event_loop,         METRIC_COUNTER, "# event loop returns" ),
                    ACTION( event_read,         METRIC_COUNTER, "# reads registered"   ),
                    ACTION( event_write,        METRIC_COUNTER, "# writes registered"  )
                }
            },
            log: init_metric! {
                log_metrics_st {
                    ACTION( log_create,     METRIC_COUNTER, "# loggers created"                ),
                    ACTION( log_create_ex,  METRIC_COUNTER, "# log create errors"              ),
                    ACTION( log_destroy,    METRIC_COUNTER, "# loggers destroyed"              ),
                    ACTION( log_curr,       METRIC_GAUGE,   "current # loggers"                ),
                    ACTION( log_open,       METRIC_COUNTER, "# files opened by loggers"        ),
                    ACTION( log_open_ex,    METRIC_COUNTER, "# logger open file errors"        ),
                    ACTION( log_write,      METRIC_COUNTER, "# log messages written"           ),
                    ACTION( log_write_byte, METRIC_COUNTER, "# bytes written by log"           ),
                    ACTION( log_write_ex,   METRIC_COUNTER, "# log write errors"               ),
                    ACTION( log_skip,       METRIC_COUNTER, "# messages not completely logged" ),
                    ACTION( log_skip_byte,  METRIC_COUNTER, "# bytes unable to be logged"      ),
                    ACTION( log_flush,      METRIC_COUNTER, "# log flushes to disk"            ),
                    ACTION( log_flush_ex,   METRIC_COUNTER, "# errors flushing to disk"        )
                }
            },
            sockio: init_metric! {
                sockio_metrics_st {
                    ACTION( buf_sock_create,    METRIC_COUNTER, "# buf sock created"           ),
                    ACTION( buf_sock_create_ex, METRIC_COUNTER, "# buf sock create exceptions" ),
                    ACTION( buf_sock_destroy,   METRIC_COUNTER, "# buf sock destroyed"         ),
                    ACTION( buf_sock_curr,      METRIC_GAUGE,   "# buf sock allocated"         ),
                    ACTION( buf_sock_borrow,    METRIC_COUNTER, "# buf sock borrowed"          ),
                    ACTION( buf_sock_borrow_ex, METRIC_COUNTER, "# buf sock borrow exceptions" ),
                    ACTION( buf_sock_return,    METRIC_COUNTER, "# buf sock returned"          ),
                    ACTION( buf_sock_active,    METRIC_GAUGE,   "# buf sock being borrowed"    )
                }
            },
            tcp: init_metric! {
                tcp_metrics_st {
                    ACTION( tcp_conn_create,    METRIC_COUNTER, "# tcp connections created"    ),
                    ACTION( tcp_conn_create_ex, METRIC_COUNTER, "# tcp conn create exceptions" ),
                    ACTION( tcp_conn_destroy,   METRIC_COUNTER, "# tcp connections destroyed"  ),
                    ACTION( tcp_conn_curr,      METRIC_GAUGE,   "# tcp conn allocated"         ),
                    ACTION( tcp_conn_borrow,    METRIC_COUNTER, "# tcp connections borrowed"   ),
                    ACTION( tcp_conn_borrow_ex, METRIC_COUNTER, "# tcp conn borrow exceptions" ),
                    ACTION( tcp_conn_return,    METRIC_COUNTER, "# tcp connections returned"   ),
                    ACTION( tcp_conn_active,    METRIC_GAUGE,   "# tcp conn being borrowed"    ),
                    ACTION( tcp_accept,         METRIC_COUNTER, "# tcp connection accepts"     ),
                    ACTION( tcp_accept_ex,      METRIC_COUNTER, "# tcp accept exceptions"      ),
                    ACTION( tcp_reject,         METRIC_COUNTER, "# tcp connection rejects"     ),
                    ACTION( tcp_reject_ex,      METRIC_COUNTER, "# tcp reject exceptions"      ),
                    ACTION( tcp_connect,        METRIC_COUNTER, "# tcp connects made"          ),
                    ACTION( tcp_connect_ex,     METRIC_COUNTER, "# tcp connect exceptions "    ),
                    ACTION( tcp_close,          METRIC_COUNTER, "# tcp connection closed"      ),
                    ACTION( tcp_recv,           METRIC_COUNTER, "# recv attempted"             ),
                    ACTION( tcp_recv_ex,        METRIC_COUNTER, "# recv exceptions"            ),
                    ACTION( tcp_recv_byte,      METRIC_COUNTER, "# bytes received"             ),
                    ACTION( tcp_send,           METRIC_COUNTER, "# send attempted"             ),
                    ACTION( tcp_send_ex,        METRIC_COUNTER, "# send exceptions"            ),
                    ACTION( tcp_send_byte,      METRIC_COUNTER, "# bytes sent"                 )
                }
            },
            timing_wheel: init_metric! {
                timing_wheel_metrics_st {
                    ACTION( timeout_event_curr,     METRIC_GAUGE,   "# timeout events allocated"   ),
                    ACTION( timeout_event_active,   METRIC_GAUGE,   "# timeout events in use"      ),
                    ACTION( timeout_event_borrow,   METRIC_COUNTER, "# timeout events borrowed"    ),
                    ACTION( timeout_event_borrow_ex,METRIC_COUNTER, "# tevents borrow errors"      ),
                    ACTION( timeout_event_return,   METRIC_COUNTER, "# timeout events returned"    ),
                    ACTION( timing_wheel_insert,    METRIC_COUNTER, "# tevent insertions"          ),
                    ACTION( timing_wheel_remove,    METRIC_COUNTER, "# tevent removal"             ),
                    ACTION( timing_wheel_event,     METRIC_GAUGE,   "# tevents in timing wheels"   ),
                    ACTION( timing_wheel_process,   METRIC_COUNTER, "# tevents processed"          ),
                    ACTION( timing_wheel_tick,      METRIC_COUNTER, "# ticks processed"            ),
                    ACTION( timing_wheel_exec,      METRIC_COUNTER, "# timing wheel executions "   )
                }
            },
        }
    }

    pub const fn num_metrics() -> usize {
        use std::mem::size_of;

        size_of::<Metrics>() / size_of::<metric>()
    }
}

unsafe impl AsMetricArray for Metrics {}

#[test]
fn test_stats_size_is_multiple_of_metric_size() {
    use ccommon_sys::metric;
    use std::mem;

    let metric_size = mem::size_of::<metric>();
    assert_eq!(mem::size_of::<Metrics>() % metric_size, 0);
}
