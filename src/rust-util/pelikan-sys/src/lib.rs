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

#![allow(
    improper_ctypes,
    non_upper_case_globals,
    non_snake_case,
    non_camel_case_types
)]

// Hidden module for stuff used by init_metric!
#[doc(hidden)]
pub mod export {
    pub use ccommon;
    pub use ccommon_sys;
}

const fn slice_to_ptr(arr: &[u8]) -> *mut i8 {
    arr.as_ptr() as *const i8 as *mut _
}

#[doc(hidden)]
#[macro_export]
macro_rules! __pelikan_sys__c_str {
    ($s:expr) => {
        concat!($s, "\0").as_bytes().as_ptr() as *const i8 as *mut _
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __pelikan_sys__stringify {
    ( $( $tt:tt )* ) => { stringify!($( $tt )*) }
}

#[doc(hidden)]
#[macro_export]
macro_rules! __pelikan_sys__init_option_single {
    [OPTION_TYPE_BOOL, $default:expr] => {
        $crate::export::ccommon_sys::option_val {
            vbool: $default
        }
    };
    [OPTION_TYPE_UINT, $default:expr] => {
        $crate::export::ccommon_sys::option_val {
            vuint: $default.into()
        }
    };
    [OPTION_TYPE_FPN, $default:expr] => {
        $crate::export::ccommon_sys::option_val {
            vfpn: $default.into()
        }
    };
    [OPTION_TYPE_STR, $default:expr] => {
        $crate::export::ccommon_sys::option_val {
            vstr: $default
        }
    };
    ($name:ident, $ty:ident, $default:expr, $desc:literal) => {
        $crate::export::ccommon_sys::option {
            name: concat!(stringify!($name), "\0").as_ptr() as *mut _,
            set: false,
            type_: $crate::export::ccommon_sys::$ty,
            default_val: __pelikan_sys__init_option_single![$ty, $default],
            val: __pelikan_sys__init_option_single![$ty, $default],
            description: concat!($desc, "\0").as_ptr() as *mut _
        }
    }
}

/// Macro to properly initialize an options struct.
#[macro_export(local_inner_macros)]
macro_rules! init_option {
    {
        $name:tt ;
        $(
            ACTION(
                $field:ident, $ty:ident, $default:expr, $desc:literal
            )
        ),* $(,)?
    } => {
        init_option! {
            $name {
                $( ACTION($field, $ty, $default, $desc) ),*
            }
        }
    };
    {
        $name:tt {
            $(
                ACTION(
                    $field:ident, $ty:ident, $default:expr, $desc:literal
                )
            ),* $(,)?
        }
    } => {
        {
            $name {
                $(
                    $field: $crate::__pelikan_sys__init_option_single!(
                        $field, $ty, $default, $desc
                    ),
                )*
            }
        }
    }
}

/// Macro to properly initialize a metrics struct.
#[macro_export(local_inner_macros)]
macro_rules! init_metric {
    [METRIC_GAUGE] => {
        $crate::export::ccommon_sys::metric_anon_union::gauge(0)
    };
    [METRIC_COUNTER] => {
        $crate::export::ccommon_sys::metric_anon_union::counter(0)
    };
    [METRIC_FPN] => {
        $crate::export::ccommon_sys::metric_anon_union::fpn(0.0)
    };
    {
        $name:tt ;
        $(
            ACTION(
                $field:ident, $ty:ident, $desc:literal
            )
        ),* $(,)?
    } => {
        init_metric! {
            $name {
                $( ACTION($field, $ty, $desc) ),*
            }
        }
    };
    {
        $name:tt {
            $(
                ACTION( $field:ident , $ty:ident, $desc:literal )
            ),* $(,)?
        }
    } => {
        $name {
            $(
                $field: $crate::export::ccommon_sys::metric {
                    name: $crate::__pelikan_sys__c_str!(
                        $crate::__pelikan_sys__stringify!($field)
                    ),
                    desc: $crate::__pelikan_sys__c_str!($desc),
                    type_: $crate::export::ccommon_sys::$ty,
                    data: init_metric![$ty]
                },
            )*
        }
    }
}

include!(concat!(env!("OUT_DIR"), "/config.rs"));

pub mod client {
    #[cfg(feature = "client-network")]
    pub mod network {
        use ccommon_sys::{buf_sock, channel_handler_st};

        include!(concat!(env!("OUT_DIR"), "/client-network.rs"));
    }
}

#[cfg(feature = "core")]
pub mod core {
    use crate::slice_to_ptr;
    use ccommon_sys::{buf, metric, option, pipe_conn, ring_array};
    use ccommon_sys::{timeout_cb_fn, timeout_event};

    pub const SERVER_PORT: &[u8] = b"12321\0";
    pub const ADMIN_PORT: &[u8] = b"9999\0";

    include!(concat!(env!("OUT_DIR"), "/core.rs"));

    unsafe impl ccommon::metric::Metrics for server_metrics_st {
        fn new() -> Self {
            init_metric! {
                Self {
                    ACTION( server_event_total,     METRIC_COUNTER, "# server events returned"      ),
                    ACTION( server_event_loop,      METRIC_COUNTER, "# server event loops returned" ),
                    ACTION( server_event_read,      METRIC_COUNTER, "# server core_read events"     ),
                    ACTION( server_event_write,     METRIC_COUNTER, "# server core_write events"    ),
                    ACTION( server_event_error,     METRIC_COUNTER, "# server core_error events"    )
                }
            }
        }
    }
    unsafe impl ccommon::metric::Metrics for worker_metrics_st {
        fn new() -> Self {
            init_metric! {
                Self {
                    ACTION( worker_event_total,     METRIC_COUNTER, "# worker events returned"      ),
                    ACTION( worker_event_loop,      METRIC_COUNTER, "# worker event loops returned" ),
                    ACTION( worker_event_read,      METRIC_COUNTER, "# worker core_read events"     ),
                    ACTION( worker_event_write,     METRIC_COUNTER, "# worker core_write events"    ),
                    ACTION( worker_event_error,     METRIC_COUNTER, "# worker core_error events"    ),
                    ACTION( worker_add_stream,      METRIC_COUNTER, "# worker adding a stream"      ),
                    ACTION( worker_ret_stream,      METRIC_COUNTER, "# worker returning a stream"   )

                }
            }
        }
    }

    unsafe impl ccommon::option::Options for admin_options_st {
        fn new() -> Self {
            init_option! {
                Self;
                ACTION( admin_host,     OPTION_TYPE_STR,    std::ptr::null_mut(),       "admin interfaces listening on"),
                ACTION( admin_port,     OPTION_TYPE_STR,    slice_to_ptr(ADMIN_PORT),   "admin port"                   ),
                ACTION( admin_timeout,  OPTION_TYPE_UINT,   ADMIN_TIMEOUT as u64,       "evwait timeout"               ),
                ACTION( admin_nevent,   OPTION_TYPE_UINT,   ADMIN_NEVENT as u64,        "evwait max nevent returned"   ),
                ACTION( admin_tw_tick,  OPTION_TYPE_UINT,   ADMIN_TW_TICK as u64,       "timing wheel tick size (ms)"  ),
                ACTION( admin_tw_cap,   OPTION_TYPE_UINT,   ADMIN_TW_CAP as u64,        "# ticks in timing wheel"      ),
                ACTION( admin_tw_ntick, OPTION_TYPE_UINT,   ADMIN_TW_NTICK as u64,      "max # ticks processed at once")
            }
        }
    }
    unsafe impl ccommon::option::Options for server_options_st {
        fn new() -> Self {
            init_option! {
                Self;
                ACTION( server_host,    OPTION_TYPE_STR,    std::ptr::null_mut(),       "interfaces listening on"      ),
                ACTION( server_port,    OPTION_TYPE_STR,    slice_to_ptr(SERVER_PORT),  "port listening on"            ),
                ACTION( server_timeout, OPTION_TYPE_UINT,   SERVER_TIMEOUT as u64,      "evwait timeout"               ),
                ACTION( server_nevent,  OPTION_TYPE_UINT,   SERVER_NEVENT as u64,       "evwait max nevent returned"   )
            }
        }
    }
    unsafe impl ccommon::option::Options for worker_options_st {
        fn new() -> Self {
            init_option! {
                Self;
                ACTION( worker_timeout, OPTION_TYPE_UINT,   WORKER_TIMEOUT as u64, "evwait timeout"               ),
                ACTION( worker_nevent,  OPTION_TYPE_UINT,   WORKER_NEVENT as u64,  "evwait max nevent returned"   )
            }
        }
    }
}

pub mod data_structure {
    use ccommon_sys::bstring;

    include!(concat!(env!("OUT_DIR"), "/ds_shared.rs"));

    #[cfg(feature = "ds_bitmap")]
    pub mod bitmap {
        include!(concat!(env!("OUT_DIR"), "/ds_bitmap.rs"));
    }

    #[cfg(feature = "ds_sarray")]
    pub mod sarray {
        include!(concat!(env!("OUT_DIR"), "/ds_sarray.rs"));
    }

    #[cfg(feature = "ds_ziplist")]
    pub mod ziplist {
        use super::blob;

        include!(concat!(env!("OUT_DIR"), "/ds_ziplist.rs"));
    }
}

pub mod storage {
    #[cfg(feature = "cuckoo")]
    pub mod cuckoo {
        use crate::time::{delta_time_i, proc_time_i};
        use ccommon_sys::{bstring, metric, option, rstatus_i};

        include!(concat!(env!("OUT_DIR"), "/cuckoo.rs"));
    }

    #[cfg(feature = "slab")]
    pub mod slab {
        use crate::time::proc_time_i;
        use ccommon_sys::{bstring, metric, option};

        include!(concat!(env!("OUT_DIR"), "/slab.rs"));
    }

    #[cfg(feature = "cdb")]
    pub mod cdb {
        use ccommon_sys::bstring;

        include!(concat!(env!("OUT_DIR"), "/cdb.rs"));
    }
}

#[cfg(feature = "time")]
pub mod time {
    use ccommon_sys::option;
    use libc::time_t;

    include!(concat!(env!("OUT_DIR"), "/time.rs"));

    unsafe impl ccommon::option::Options for time_options_st {
        fn new() -> Self {
            init_option! {
                Self;
                ACTION(time_type, OPTION_TYPE_UINT, TIME_UNIX as u64, "Expiry timestamp mode")
            }
        }
    }
}

#[cfg(feature = "util")]
pub mod util {
    use ccommon_sys::{metric, rstatus_i};
    use libc::addrinfo;

    include!(concat!(env!("OUT_DIR"), "/util.rs"));

    unsafe impl ccommon::metric::Metrics for procinfo_metrics_st {
        fn new() -> Self {
            init_metric! {
                Self {
                    ACTION( pid,            METRIC_GAUGE,   "pid of current process"   ),
                    ACTION( time,           METRIC_COUNTER, "unix time in seconds"     ),
                    ACTION( uptime,         METRIC_COUNTER, "process uptime in ms"     ),
                    ACTION( version,        METRIC_COUNTER, "version as an int"        ),
                    ACTION( ru_stime,       METRIC_FPN,     "system CPU time"          ),
                    ACTION( ru_utime,       METRIC_FPN,     "user CPU time"            ),
                    ACTION( ru_maxrss,      METRIC_GAUGE,   "max RSS size"             ),
                    ACTION( ru_ixrss,       METRIC_GAUGE,   "text memory size"         ),
                    ACTION( ru_idrss,       METRIC_GAUGE,   "data memory size"         ),
                    ACTION( ru_isrss,       METRIC_GAUGE,   "stack memory size"        ),
                    ACTION( ru_minflt,      METRIC_COUNTER, "pagefault w/o I/O"        ),
                    ACTION( ru_majflt,      METRIC_COUNTER, "pagefault w/ I/O"         ),
                    ACTION( ru_nswap,       METRIC_COUNTER, "# times swapped"          ),
                    ACTION( ru_inblock,     METRIC_COUNTER, "real FS input"            ),
                    ACTION( ru_oublock,     METRIC_COUNTER, "real FS output"           ),
                    ACTION( ru_msgsnd,      METRIC_COUNTER, "# IPC messages sent"      ),
                    ACTION( ru_msgrcv,      METRIC_COUNTER, "# IPC messages received"  ),
                    ACTION( ru_nsignals,    METRIC_COUNTER, "# signals delivered"      ),
                    ACTION( ru_nvcsw,       METRIC_COUNTER, "# voluntary CS"           ),
                    ACTION( ru_nivcsw,      METRIC_COUNTER, "# involuntary CS"         )
                }
            }
        }
    }
}

pub mod protocol {
    #[cfg(feature = "protocol_resp")]
    pub mod resp {
        use ccommon_sys::{array, bstring, buf, metric, option};

        include!(concat!(env!("OUT_DIR"), "/protocol_resp.rs"));
    }

    #[cfg(feature = "protocol_resp_tw")]
    pub mod resp_tw {
        use ccommon_sys::{array, bstring, buf, metric, option};

        include!(concat!(env!("OUT_DIR"), "/protocol_resp_tw.rs"));
    }

    #[cfg(feature = "protocol_memcache")]
    pub mod memcache {
        use ccommon_sys::{array, bstring, buf, metric, option};

        include!(concat!(env!("OUT_DIR"), "/protocol_memcache.rs"));
    }

    #[cfg(feature = "protocol_ping")]
    pub mod ping {
        use ccommon_sys::{buf, metric};

        include!(concat!(env!("OUT_DIR"), "/protocol_ping.rs"));

        unsafe impl ccommon::metric::Metrics for parse_req_metrics_st {
            fn new() -> Self {
                init_metric! {
                    Self {
                        ACTION( request_parse,      METRIC_COUNTER, "# requests parsed"    ),
                        ACTION( request_parse_ex,   METRIC_COUNTER, "# parsing error"      )
                    }
                }
            }
        }
        unsafe impl ccommon::metric::Metrics for parse_rsp_metrics_st {
            fn new() -> Self {
                init_metric! {
                    Self {
                        ACTION( response_parse,     METRIC_COUNTER, "# responses parsed"   ),
                        ACTION( response_parse_ex,  METRIC_COUNTER, "# rsp parsing error"  ),

                    }
                }
            }
        }

        unsafe impl ccommon::metric::Metrics for compose_req_metrics_st {
            fn new() -> Self {
                init_metric! {
                    Self {
                        ACTION( request_compose,        METRIC_COUNTER, "# requests composed"  ),
                        ACTION( request_compose_ex,     METRIC_COUNTER, "# composing error"    )
                    }
                }
            }
        }
        unsafe impl ccommon::metric::Metrics for compose_rsp_metrics_st {
            pub const fn new() -> Self {
                init_metric! {
                    Self {
                        ACTION( response_compose,       METRIC_COUNTER, "# responses composed" ),
                        ACTION( response_compose_ex,    METRIC_COUNTER, "# rsp composing error")
                    }
                }
            }
        }
    }

    #[cfg(feature = "protocol_admin")]
    pub mod admin {
        use ccommon_sys::{bstring, buf, metric};

        pub const METRIC_END_LEN: usize = std::mem::size_of::<[u8; 6]>() - 1;

        include!(concat!(env!("OUT_DIR"), "/protocol_admin.rs"));

        #[test]
        fn metric_end_len_valid() {
            assert_eq!(METRIC_END_LEN, std::mem::size_of_val(METRIC_END) - 1);
        }
    }
}
