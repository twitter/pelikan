use ccommon::util::AsOptionArray;
use ccommon_sys::{
    array_options_st, buf_options_st, dbuf_options_st, debug_options_st, option, sockio_options_st,
    tcp_options_st,
};
use pelikan_sys::{
    core::{admin_options_st, server_options_st, worker_options_st},
    init_option,
    time::time_options_st,
};
use std::ptr::null_mut;

#[repr(C)]
pub struct PingServerOptions {
    pub daemonize: option,
    pub pid_filename: option,
    pub dlog_intvl: option,
}

#[rustfmt::skip]
#[repr(C)]
pub struct Settings {
    // top-level
    pub pingserver: PingServerOptions,
    // application modules
    pub admin:      admin_options_st,
    pub server:     server_options_st,
    pub worker:     worker_options_st,
    pub time:       time_options_st,
    // ccommon libraries
    pub array:      array_options_st,
    pub buf:        buf_options_st,
    pub dbuf:       dbuf_options_st,
    pub debug:      debug_options_st,
    pub sockio:     sockio_options_st,
    pub tcp:        tcp_options_st
}

impl Settings {
    pub const fn new() -> Self {
        use ccommon_sys::{
            BUFSOCK_POOLSIZE, BUF_DEFAULT_SIZE, BUF_POOLSIZE, DBUF_DEFAULT_MAX, DEBUG_LOG_FILE,
            DEBUG_LOG_LEVEL, DEBUG_LOG_NBUF, NELEM_DELTA, TCP_BACKLOG, TCP_POOLSIZE,
        };

        Settings {
            pingserver: init_option! {
                PingServerOptions {
                    ACTION( daemonize,      OPTION_TYPE_BOOL,   false,      "daemonize the process"        ),
                    ACTION( pid_filename,   OPTION_TYPE_STR,    null_mut(), "file storing the pid"         ),
                    ACTION( dlog_intvl,     OPTION_TYPE_UINT,   500,        "debug log flush interval(ms)" )
                }
            },
            admin: admin_options_st::new(),
            server: server_options_st::new(),
            worker: worker_options_st::new(),
            time: time_options_st::new(),
            array: init_option! {
                array_options_st;
                ACTION( array_nelem_delta,  OPTION_TYPE_UINT,   NELEM_DELTA as u64,      "max nelem delta during expansion" )
            },
            buf: init_option! {
                buf_options_st;
                ACTION( buf_init_size,  OPTION_TYPE_UINT,   BUF_DEFAULT_SIZE as u64,   "init buf size incl header" ),
                ACTION( buf_poolsize,   OPTION_TYPE_UINT,   BUF_POOLSIZE as u64,       "buf pool size"             )
            },
            dbuf: init_option! {
                dbuf_options_st;
                ACTION( dbuf_max_power,      OPTION_TYPE_UINT,  DBUF_DEFAULT_MAX as u64,   "max number of doubles")
            },
            debug: init_option! {
                debug_options_st;
                ACTION( debug_log_level, OPTION_TYPE_UINT, DEBUG_LOG_LEVEL as u64,  "debug log level"     ),
                ACTION( debug_log_file,  OPTION_TYPE_STR,  DEBUG_LOG_FILE,   "debug log file"      ),
                ACTION( debug_log_nbuf,  OPTION_TYPE_UINT, DEBUG_LOG_NBUF as u64,   "debug log buf size"  )
            },
            sockio: init_option! {
                sockio_options_st;
                ACTION( buf_sock_poolsize,  OPTION_TYPE_UINT,   BUFSOCK_POOLSIZE as u64,   "buf_sock limit" )
            },
            tcp: init_option! {
                tcp_options_st;
                ACTION( tcp_backlog,    OPTION_TYPE_UINT,   TCP_BACKLOG as u64,    "tcp conn backlog limit" ),
                ACTION( tcp_poolsize,   OPTION_TYPE_UINT,   TCP_POOLSIZE as u64,   "tcp conn pool size"     )
            },
        }
    }

    #[allow(dead_code)]
    pub const fn num_settings() -> usize {
        use std::mem::size_of;

        size_of::<Settings>() / size_of::<option>()
    }
}

unsafe impl AsOptionArray for Settings {}

#[test]
fn test_settings_size_is_multiple_of_option_size() {
    use ccommon_sys::option;
    use std::mem;

    let option_size = mem::size_of::<option>();
    assert_eq!(mem::size_of::<Settings>() % option_size, 0);
}
