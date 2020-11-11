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

use ccommon::Metrics;
use ccommon_sys::*;
use pelikan_sys::{
    core::server_metrics_st, protocol::memcache::*, storage::slab::slab_metrics_st,
    util::procinfo_metrics_st,
};

use rustcore::{admin::AdminMetrics, worker::WorkerMetrics, TcpListenerMetrics};

use crate::memcached::sys::process_metrics_st;

#[rustfmt::skip]
#[repr(C)]
#[derive(Metrics)]
pub struct Metrics {
    // Perf info
    pub procinfo:       procinfo_metrics_st,

    // Application Modules
    pub parse_req:      parse_req_metrics_st,
    pub compose_rsp:    compose_rsp_metrics_st,
    pub server:         server_metrics_st,
    pub admin:          AdminMetrics,
    pub listener:       TcpListenerMetrics,
    pub worker:         WorkerMetrics,
    pub klog:           klog_metrics_st,
    pub slab:           slab_metrics_st,
    pub process:        process_metrics_st,
    pub request:        request_metrics_st,
    pub response:       response_metrics_st,

    // Common libraries
    pub buf:            buf_metrics_st,
    pub dbuf:           dbuf_metrics_st,
    pub log:            log_metrics_st,
    pub sockio:         sockio_metrics_st,
}

unsafe impl Send for Metrics {}
unsafe impl Sync for Metrics {}

#[test]
fn test_stats_size_is_multiple_of_metric_size() {
    use ccommon_sys::metric;
    use std::mem;

    let metric_size = mem::size_of::<metric>();
    assert_eq!(mem::size_of::<Metrics>() % metric_size, 0);
}
