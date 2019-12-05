pub(crate) mod sys {
    #![allow(dead_code, non_camel_case_types)]

    use ccommon::metric::Metrics;
    use ccommon::option::Options;
    use ccommon_sys::{buf, metric, option};

    include!(concat!(env!("OUT_DIR"), "/memcached.rs"));

    unsafe impl Options for process_options_st {
        fn new() -> Self {
            init_option! {
                ACTION( allow_flush,   OPTION_TYPE_BOOL, ALLOW_FLUSH != 0,  "allow flush_all"         ),
                ACTION( prefill,       OPTION_TYPE_BOOL, PREFILL != 0,      "prefill slabs with data" ),
                ACTION( prefill_ksize, OPTION_TYPE_UINT, PREFILL_KSIZE,     "prefill key size"        ),
                ACTION( prefill_vsize, OPTION_TYPE_UINT, PREFILL_VSIZE,     "prefill val size"        ),
                ACTION( prefill_nkey,  OPTION_TYPE_UINT, PREFILL_NKEY,      "prefill keys inserted"   )
            }
        }
    }
    unsafe impl Metrics for process_metrics_st {
        fn new() -> Self {
            init_metric! {
                ACTION( process_req,       METRIC_COUNTER, "# requests processed"  ),
                ACTION( process_ex,        METRIC_COUNTER, "# processing error"    ),
                ACTION( process_server_ex, METRIC_COUNTER, "# internal error"      ),
                ACTION( get,               METRIC_COUNTER, "# get requests"        ),
                ACTION( get_key,           METRIC_COUNTER, "# keys by get"         ),
                ACTION( get_key_hit,       METRIC_COUNTER, "# key hits by get"     ),
                ACTION( get_key_miss,      METRIC_COUNTER, "# key misses by get"   ),
                ACTION( get_ex,            METRIC_COUNTER, "# get errors"          ),
                ACTION( gets,              METRIC_COUNTER, "# gets requests"       ),
                ACTION( gets_key,          METRIC_COUNTER, "# keys by gets"        ),
                ACTION( gets_key_hit,      METRIC_COUNTER, "# key hits by gets"    ),
                ACTION( gets_key_miss,     METRIC_COUNTER, "# key misses by gets"  ),
                ACTION( gets_ex,           METRIC_COUNTER, "# gets errors"         ),
                ACTION( delete,            METRIC_COUNTER, "# delete requests"     ),
                ACTION( delete_deleted,    METRIC_COUNTER, "# delete successes"    ),
                ACTION( delete_notfound,   METRIC_COUNTER, "# delete not_founds"   ),
                ACTION( set,               METRIC_COUNTER, "# set requests"        ),
                ACTION( set_stored,        METRIC_COUNTER, "# set successes"       ),
                ACTION( set_ex,            METRIC_COUNTER, "# set errors"          ),
                ACTION( add,               METRIC_COUNTER, "# add requests"        ),
                ACTION( add_stored,        METRIC_COUNTER, "# add successes"       ),
                ACTION( add_notstored,     METRIC_COUNTER, "# add failures"        ),
                ACTION( add_ex,            METRIC_COUNTER, "# add errors"          ),
                ACTION( replace,           METRIC_COUNTER, "# replace requests"    ),
                ACTION( replace_stored,    METRIC_COUNTER, "# replace successes"   ),
                ACTION( replace_notstored, METRIC_COUNTER, "# replace failures"    ),
                ACTION( replace_ex,        METRIC_COUNTER, "# replace errors"      ),
                ACTION( cas,               METRIC_COUNTER, "# cas requests"        ),
                ACTION( cas_stored,        METRIC_COUNTER, "# cas successes"       ),
                ACTION( cas_exists,        METRIC_COUNTER, "# cas bad values"      ),
                ACTION( cas_notfound,      METRIC_COUNTER, "# cas not_founds"      ),
                ACTION( cas_ex,            METRIC_COUNTER, "# cas errors"          ),
                ACTION( incr,              METRIC_COUNTER, "# incr requests"       ),
                ACTION( incr_stored,       METRIC_COUNTER, "# incr successes"      ),
                ACTION( incr_notfound,     METRIC_COUNTER, "# incr not_founds"     ),
                ACTION( incr_ex,           METRIC_COUNTER, "# incr errors"         ),
                ACTION( decr,              METRIC_COUNTER, "# decr requests"       ),
                ACTION( decr_stored,       METRIC_COUNTER, "# decr successes"      ),
                ACTION( decr_notfound,     METRIC_COUNTER, "# decr not_founds"     ),
                ACTION( decr_ex,           METRIC_COUNTER, "# decr errors"         ),
                ACTION( append,            METRIC_COUNTER, "# append requests"     ),
                ACTION( append_stored,     METRIC_COUNTER, "# append successes"    ),
                ACTION( append_notstored,  METRIC_COUNTER, "# append not_founds"   ),
                ACTION( append_ex,         METRIC_COUNTER, "# append errors"       ),
                ACTION( prepend,           METRIC_COUNTER, "# prepend requests"    ),
                ACTION( prepend_stored,    METRIC_COUNTER, "# prepend successes"   ),
                ACTION( prepend_notstored, METRIC_COUNTER, "# prepend not_founds"  ),
                ACTION( prepend_ex,        METRIC_COUNTER, "# prepend errors"      ),
                ACTION( flush,             METRIC_COUNTER, "# flush_all requests"  ),
            }
        }
    }
}

use ccommon::buf::OwnedBuf;
use ccommon_sys::buf;
use rustcore::util::{read_buf, write_buf};
use rustcore::worker::WorkerMetrics;
use tokio::io::{AsyncRead, AsyncWrite};

use std::rc::Rc;

#[allow(dead_code)]
pub async fn memcached_worker<'a, S>(
    _: Rc<()>,
    stream: &'a mut S,
    rbuf: &'a mut OwnedBuf,
    wbuf: &'a mut OwnedBuf,
    metrics: &'static WorkerMetrics,
) where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let mut data = std::ptr::null_mut();

    loop {
        if let Err(_) = read_buf(stream, rbuf, metrics).await {
            break;
        }

        let ret = unsafe {
            let wbuf_ptr = wbuf as *mut _ as *mut *mut buf;
            let rbuf_ptr = rbuf as *mut _ as *mut *mut buf;

            sys::twemcache_process_read(rbuf_ptr, wbuf_ptr, &mut data)
        };

        if ret < 0 {
            break;
        };

        if wbuf.read_size() > 0 {
            if let Err(_) = write_buf(stream, wbuf, metrics).await {
                unsafe {
                    let wbuf_ptr = wbuf as *mut _ as *mut *mut buf;
                    let rbuf_ptr = rbuf as *mut _ as *mut *mut buf;

                    if sys::twemcache_process_write(rbuf_ptr, wbuf_ptr, &mut data) < 0 {
                        break;
                    }
                }
            }
        }
    }

    let wbuf_ptr = wbuf as *mut _ as *mut *mut buf;
    let rbuf_ptr = rbuf as *mut _ as *mut *mut buf;

    unsafe {
        sys::twemcache_process_error(rbuf_ptr, wbuf_ptr, &mut data);
    }
}
