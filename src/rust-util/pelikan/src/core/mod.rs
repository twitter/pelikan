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

//! Wrapper for the pelikan `core` module.

pub mod admin;

use ccommon::buf::OwnedBuf;
use ccommon_sys::buf;
use pelikan_sys::core::data_processor;

use std::os::raw::c_int;
use std::ffi::c_void;

#[repr(C)]
pub enum DataProcessorError {
    Error = -1,
}

/// Methods for handling messages on the data port
pub trait DataProcessor {
    /// Per-socket state.
    ///
    /// This field may not implement drop since there is no
    /// guarantee that it will be dropped properly
    type SockState: Copy;

    fn read(
        &mut self,
        rbuf: &mut OwnedBuf,
        wbuf: &mut OwnedBuf,
        state: &mut Option<&mut Self::SockState>,
    ) -> Result<(), DataProcessorError>;

    fn write(
        &mut self,
        rbuf: &mut OwnedBuf,
        wbuf: &mut OwnedBuf,
        state: &mut Option<&mut Self::SockState>,
    ) -> Result<(), DataProcessorError>;

    fn error(
        &mut self,
        rbuf: &mut OwnedBuf,
        wbuf: &mut OwnedBuf,
        state: &mut Option<&mut Self::SockState>,
    ) -> Result<(), DataProcessorError>;
}

unsafe extern "C" fn read_wrapper<T: DataProcessor>(
    rbuf: *mut *mut buf,
    wbuf: *mut *mut buf,
    data: *mut *mut c_void,
) -> c_int {
    assert!(!rbuf.is_null());
    assert!(!wbuf.is_null());
    assert!(!data.is_null());
    assert!(!(*rbuf).is_null());
    assert!(!(*wbuf).is_null());
    assert!(!DATA_PTR.is_null());

    let ptr = DATA_PTR as *mut T;

    let res = (*ptr).read(
        &mut *(rbuf as *mut OwnedBuf),
        &mut *(wbuf as *mut OwnedBuf),
        &mut *(data as *mut _),
    );

    match res {
        Ok(_) => 0,
        Err(e) => e as c_int,
    }
}

unsafe extern "C" fn write_wrapper<T: DataProcessor>(
    rbuf: *mut *mut buf,
    wbuf: *mut *mut buf,
    data: *mut *mut c_void,
) -> c_int {
    assert!(!rbuf.is_null());
    assert!(!wbuf.is_null());
    assert!(!data.is_null());
    assert!(!(*rbuf).is_null());
    assert!(!(*wbuf).is_null());
    assert!(!DATA_PTR.is_null());

    let ptr = DATA_PTR as *mut T;

    let res = (*ptr).write(
        &mut *(rbuf as *mut OwnedBuf),
        &mut *(wbuf as *mut OwnedBuf),
        &mut *(data as *mut _),
    );

    match res {
        Ok(_) => 0,
        Err(e) => e as c_int,
    }
}

unsafe extern "C" fn error_wrapper<T: DataProcessor>(
    rbuf: *mut *mut buf,
    wbuf: *mut *mut buf,
    data: *mut *mut c_void,
) -> c_int {
    assert!(!rbuf.is_null());
    assert!(!wbuf.is_null());
    assert!(!data.is_null());
    assert!(!(*rbuf).is_null());
    assert!(!(*wbuf).is_null());
    assert!(!DATA_PTR.is_null());

    let ptr = DATA_PTR as *mut T;

    let res = (*ptr).error(
        &mut *(rbuf as *mut OwnedBuf),
        &mut *(wbuf as *mut OwnedBuf),
        &mut *(data as *mut _)
    );

    match res {
        Ok(_) => 0,
        Err(e) => e as c_int,
    }
}

static mut DATA_PTR: *mut () = std::ptr::null_mut();

/// Start the server on the data port.
/// 
/// # Safety
/// It is unsafe to call this function concurrently.
/// This function is not reentrant.
pub unsafe fn core_run<DP: DataProcessor>(mut dp: DP) {
    let mut processor = data_processor {
        read: Some(read_wrapper::<DP>),
        write: Some(write_wrapper::<DP>),
        error: Some(error_wrapper::<DP>),
    };

    assert!(DATA_PTR.is_null());
    DATA_PTR = &mut dp as *mut _ as *mut ();

    pelikan_sys::core::core_run(&mut processor as *mut _ as *mut c_void);

    DATA_PTR = std::ptr::null_mut();
}
