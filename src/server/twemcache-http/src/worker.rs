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

use ccommon_sys::bstring;
use pelikan::storage::slab::ItemError;
use pelikan_sys::protocol::memcache::DATAFLAG_SIZE;
use pelikan_sys::storage::slab::*;
use pelikan_sys::time::time_convert_proc_sec;

use std::marker::PhantomData;
use std::time::{Duration, SystemTime};

pub struct Worker {
    // Ensure that worker is !Send
    _marker: PhantomData<*const ()>,
}

impl Worker {
    pub fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }

    pub fn get(&mut self, key: &[u8]) -> Option<&mut item> {
        unsafe {
            let mut bstr = bstring {
                data: key.as_ptr() as *mut u8 as *mut i8,
                len: key.len() as u32,
            };

            let item = item_get(&mut bstr as *mut bstring);

            if item.is_null() {
                None
            } else {
                Some(&mut *item)
            }
        }
    }

    pub fn put<'a>(
        &'a mut self,
        key: &[u8],
        val: &[u8],
        expiry: SystemTime,
        dataflag: u32,
    ) -> Result<&'a mut item, ItemError> {
        unsafe {
            let mut item = std::ptr::null_mut();

            let mut key_bstr = bstring {
                data: key.as_ptr() as *mut i8,
                len: key.len() as u32,
            };
            let mut val_bstr = bstring {
                data: val.as_ptr() as *mut i8,
                len: val.len() as u32,
            };

            let timestamp: Duration = expiry
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or(Duration::from_secs(0));

            let status = item_reserve(
                &mut item,
                &mut key_bstr,
                &mut val_bstr,
                val.len() as u32,
                DATAFLAG_SIZE as u8,
                time_convert_proc_sec(timestamp.as_secs() as i32),
            );

            if status != ITEM_OK {
                return Err(ItemError::from(status));
            }

            assert!(!item.is_null());
            *(item_optional(item) as *mut u32) = dataflag;

            item_insert(item, &key_bstr);

            Ok(&mut *item)
        }
    }

    pub fn delete(&mut self, key: &[u8]) -> bool {
        unsafe {
            let key = bstring {
                data: key.as_ptr() as *mut i8,
                len: key.len() as u32,
            };

            item_delete(&key)
        }
    }
}
