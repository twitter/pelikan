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

use crate::stats::Metrics;

use ccommon::metric::MetricExt;
use pelikan::protocol::{admin::AdminProtocol, Protocol, StatefulProtocol};
use pelikan_sys::protocol::admin::{
    METRIC_END_LEN, METRIC_PRINT_LEN, REQ_QUIT, REQ_STATS, REQ_VERSION, RSP_GENERIC, RSP_INVALID,
    VERSION_PRINTED,
};
use rustcore::{Action, AdminHandler};

pub struct Handler<'a> {
    stats: &'a Metrics,
    buf: Vec<u8>,
}

impl<'a> Handler<'a> {
    pub fn new(stats: &'a Metrics) -> Self {
        info!("setting up the pingserver::admin module");

        let cap = METRIC_PRINT_LEN as usize * Metrics::num_metrics() + METRIC_END_LEN;
        let vec = vec![0; cap];

        Self { stats, buf: vec }
    }
}

impl<'a> AdminHandler for Handler<'a> {
    type Protocol = AdminProtocol;

    fn process_request<'de>(
        &mut self,
        req: <AdminProtocol as Protocol<'de>>::Request,
        rsp_st: &mut <AdminProtocol as StatefulProtocol>::ResponseState,
    ) -> Action<'de, AdminProtocol> {
        use ccommon_sys::*;
        use pelikan_sys::protocol::admin::print_stats;
        use pelikan_sys::util::procinfo_update;
        use std::os::raw::{c_char, c_uint};

        unsafe {
            rsp_st.type_ = RSP_GENERIC;

            match (*req).type_ {
                REQ_QUIT => return Action::Close,
                REQ_STATS => {
                    procinfo_update();
                    rsp_st.data.data = self.buf.as_mut_ptr() as *mut c_char;
                    rsp_st.data.len = print_stats(
                        self.buf.as_mut_ptr() as *mut c_char,
                        self.buf.len(),
                        self.stats.as_ptr() as *mut metric,
                        Metrics::num_metrics() as c_uint,
                    ) as u32;
                }
                REQ_VERSION => {
                    rsp_st.data.data = (&VERSION_PRINTED[..]).as_ptr() as *mut i8;
                    rsp_st.data.len = (&VERSION_PRINTED[..]).len() as u32
                }
                _ => {
                    rsp_st.type_ = RSP_INVALID;
                }
            }
        }

        Action::Respond(rsp_st as *const _)
    }
}
