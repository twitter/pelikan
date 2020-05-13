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

use crate::slice_to_ptr;
use crate::time::proc_time_i;
use ccommon_sys::{bstring, metric, option};

use std::convert::TryInto;

include!(concat!(env!("OUT_DIR"), "/slab.rs"));

pub const HASH_POWER: u32 = 16;
pub const SLAB_EVICT_OPT: u32 = EVICT_RS;
pub const SLAB_PROFILE: *mut i8 = std::ptr::null_mut();
pub const SLAB_DATAPOOL: *mut i8 = std::ptr::null_mut();

pub const ITEM_CAS_SIZE: usize = std::mem::size_of::<u64>();

pub unsafe fn item_cas_size() -> usize {
    if use_cas {
        ITEM_CAS_SIZE
    } else {
        0
    }
}

pub unsafe fn item_optional(it: *mut item) -> *mut libc::c_void {
    (*it).end[..].as_ptr().wrapping_add(item_cas_size()) as *mut _
}

pub unsafe fn item_data(it: *mut item) -> *mut libc::c_void {
    let ptr = if (*it).is_raligned() != 0 {
        (it as *const i8)
            .wrapping_add(slabclass[(*it).id as usize].size.try_into().unwrap())
            .wrapping_sub((*it).vlen() as usize)
    } else {
        (*it).end[..]
            .as_ptr()
            .wrapping_add(item_cas_size())
            .wrapping_add((*it).olen as usize)
            .wrapping_add((*it).klen as usize)
    };

    ptr as *mut libc::c_void
}

unsafe impl ccommon::metric::Metrics for slab_metrics_st {
    fn new() -> Self {
        init_metric! {
            ACTION( slab_req,           METRIC_COUNTER, "# req for new slab"       ),
            ACTION( slab_req_ex,        METRIC_COUNTER, "# slab get exceptions"    ),
            ACTION( slab_evict,         METRIC_COUNTER, "# slabs evicted"          ),
            ACTION( slab_memory,        METRIC_GAUGE,   "memory allocated to slab" ),
            ACTION( slab_curr,          METRIC_GAUGE,   "# currently active slabs" ),
            ACTION( item_curr,          METRIC_GAUGE,   "# current items"          ),
            ACTION( item_alloc,         METRIC_COUNTER, "# items allocated"        ),
            ACTION( item_alloc_ex,      METRIC_COUNTER, "# item alloc errors"      ),
            ACTION( item_dealloc,       METRIC_COUNTER, "# items de-allocated"     ),
            ACTION( item_linked_curr,   METRIC_GAUGE,   "# current items, linked"  ),
            ACTION( item_link,          METRIC_COUNTER, "# items inserted to HT"   ),
            ACTION( item_unlink,        METRIC_COUNTER, "# items removed from HT"  ),
            ACTION( item_keyval_byte,   METRIC_GAUGE,   "key+val in bytes, linked" ),
            ACTION( item_val_byte,      METRIC_GAUGE,   "value only in bytes"      )
        }
    }
}

unsafe impl ccommon::option::Options for slab_options_st {
    fn new() -> Self {
        init_option! {
            ACTION( slab_size,              OPTION_TYPE_UINT,   SLAB_SIZE,           "Slab size"                     ),
            ACTION( slab_mem,               OPTION_TYPE_UINT,   SLAB_MEM,            "Max memory by slabs (byte)"    ),
            ACTION( slab_prealloc,          OPTION_TYPE_BOOL,   SLAB_PREALLOC != 0,  "Pre-allocate slabs at setup"   ),
            ACTION( slab_evict_opt,         OPTION_TYPE_UINT,   SLAB_EVICT_OPT,      "Eviction strategy"             ),
            ACTION( slab_use_freeq,         OPTION_TYPE_BOOL,   SLAB_USE_FREEQ != 0, "Use items in free queue?"      ),
            ACTION( slab_profile,           OPTION_TYPE_STR,    SLAB_PROFILE,        "Specify entire slab profile"   ),
            ACTION( slab_item_min,          OPTION_TYPE_UINT,   ITEM_SIZE_MIN,       "Minimum item size"             ),
            ACTION( slab_item_max,          OPTION_TYPE_UINT,   SLAB_SIZE - offset_of!(slab, data) as u32,       "Maximum item size"             ),
            ACTION( slab_item_growth,       OPTION_TYPE_FPN,    ITEM_FACTOR,         "Slab class growth factor"      ),
            ACTION( slab_item_max_ttl,      OPTION_TYPE_UINT,   ITEM_MAX_TTL,        "Max ttl in seconds"            ),
            ACTION( slab_use_cas,           OPTION_TYPE_BOOL,   SLAB_USE_CAS != 0,   "Store CAS value in item"       ),
            ACTION( slab_hash_power,        OPTION_TYPE_UINT,   HASH_POWER,          "Power for lookup hash table"   ),
            ACTION( slab_datapool,          OPTION_TYPE_STR,    SLAB_DATAPOOL,       "Path to data pool"             ),
            ACTION( slab_datapool_name,     OPTION_TYPE_STR,    slice_to_ptr(SLAB_DATAPOOL_NAME),  "Slab data pool name"           ),
            ACTION( slab_datapool_prefault, OPTION_TYPE_BOOL,   SLAB_PREFAULT != 0,  "Prefault data pool"            )
        }
    }
}

unsafe impl ccommon::metric::Metrics for perslab_metrics_st {
    fn new() -> Self {
        init_metric! {
            ACTION( chunk_size,         METRIC_GAUGE,   "# byte per item cunk" ),
            ACTION( item_keyval_byte,   METRIC_GAUGE,   "keyval stored (byte) "),
            ACTION( item_val_byte,      METRIC_GAUGE,   "value portion of data"),
            ACTION( item_curr,          METRIC_GAUGE,   "# items stored"       ),
            ACTION( item_free,          METRIC_GAUGE,   "# free items"         ),
            ACTION( slab_curr,          METRIC_GAUGE,   "# slabs"              )
        }
    }
}
