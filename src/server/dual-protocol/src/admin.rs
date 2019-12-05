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

use crate::metrics::Metrics;

use ccommon::metric::MetricExt;
use pelikan::protocol::{admin::AdminProtocol, Protocol};
use pelikan_sys::protocol::admin::*;
use rustcore::{Action, AdminHandler};

const METRIC_FMT: *const i8 = " %s %s\0".as_ptr() as *const i8;

pub struct Handler<'a> {
    stats: &'a Metrics,
    buf: Vec<u8>,
}

impl<'a> Handler<'a> {
    pub fn new(stats: &'a Metrics) -> Self {
        use ccommon_sys::option;
        use pelikan_sys::storage::slab::{perslab, SLABCLASS_MAX_ID};
        use std::mem::{size_of, size_of_val};

        info!("setting up the dual::admin module");

        let nslab_metrics =
            unsafe { size_of_val(&perslab[0]) / size_of::<option>() * SLABCLASS_MAX_ID as usize };
        let nmetrics = Metrics::num_metrics();
        let cap = METRIC_PRINT_LEN as usize * nslab_metrics.max(nmetrics) + METRIC_END_LEN;
        let vec = vec![0; cap];

        Self { stats, buf: vec }
    }

    unsafe fn slab_stats(&mut self, rsp: &mut <AdminProtocol as Protocol>::Response) {
        use ccommon_sys::metric_print;
        use pelikan_sys::storage::slab::{perslab, perslab_metrics_st};
        use std::io::Write;
        use std::os::raw::{c_char, c_uint};

        self.buf.clear();

        for (id, metrics) in perslab.iter_mut().enumerate() {
            let _ = write!(&mut self.buf, "CLASS: {}\r\n", id);

            let slice = std::slice::from_raw_parts_mut(
                metrics.as_mut_ptr(),
                perslab_metrics_st::num_metrics(),
            );

            for metric in slice {
                let offset = metric_print(
                    self.buf.as_mut_ptr().wrapping_add(self.buf.len()) as *mut _,
                    self.buf.capacity() - self.buf.len(),
                    METRIC_FMT as *mut c_char,
                    metric,
                );

                self.buf.set_len(self.buf.len() + offset);
            }

            let _ = write!(&mut self.buf, "\r\n");
        }

        let _ = write!(&mut self.buf, "END\r\n");

        rsp.data.data = self.buf.as_mut_ptr() as *mut c_char;
        rsp.data.len = self.buf.len() as c_uint;
    }

    unsafe fn default_stats(&mut self, rsp: &mut <AdminProtocol as Protocol>::Response) {
        use ccommon_sys::*;
        use pelikan_sys::util::procinfo_update;
        use std::os::raw::{c_char, c_uint};

        self.buf.set_len(self.buf.capacity());

        procinfo_update();
        rsp.data.data = self.buf.as_mut_ptr() as *mut c_char;
        rsp.data.len = print_stats(
            self.buf.as_mut_ptr() as *mut c_char,
            self.buf.len(),
            self.stats.as_ptr() as *mut metric,
            Metrics::num_metrics() as c_uint,
        ) as u32;

        self.buf.set_len(0);
    }
}

impl<'a> AdminHandler for Handler<'a> {
    type Protocol = AdminProtocol;

    fn process_request<'de>(
        &mut self,
        req: &mut <AdminProtocol as Protocol>::Request,
        rsp: &mut <AdminProtocol as Protocol>::Response,
    ) -> Action {
        use ccommon::bstring::BStr;

        unsafe {
            rsp.type_ = RSP_GENERIC;

            match (*req).type_ {
                REQ_QUIT => return Action::Close,
                REQ_STATS => {
                    let slice = &BStr::from_ptr(&mut (*req).arg)[..];

                    if slice.is_empty() {
                        self.default_stats(rsp);
                    } else if slice == b" slab" {
                        self.slab_stats(rsp);
                    } else {
                        return Action::Close;
                    }
                }
                REQ_VERSION => {
                    rsp.data.data = (&VERSION_PRINTED[..]).as_ptr() as *mut i8;
                    rsp.data.len = (&VERSION_PRINTED[..]).len() as u32
                }
                _ => {
                    rsp.type_ = RSP_INVALID;
                }
            }
        }

        Action::Respond
    }
}
