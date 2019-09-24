pub mod admin;

use ccommon::buf::Buf;
use ccommon_sys::buf;

use pelikan_sys::core::data_processor;

use libc::c_int;
use std::ffi::c_void;
use std::mem::MaybeUninit;

#[repr(C)]
pub enum DataProcessorError {
    Error = -1,
}

/// Methods for handling messages on the data port
pub trait DataProcessor {
    /// Per-socket state.
    ///
    /// This field may not implement drop since there is no
    /// gaurantee that it will be dropped properly
    type SockState: Copy;

    fn read(
        rbuf: &mut Buf,
        wbuf: &mut Buf,
        state: &mut *mut MaybeUninit<Self::SockState>,
    ) -> Result<(), DataProcessorError>;
    fn write(
        rbuf: &mut Buf,
        wbuf: &mut Buf,
        state: &mut *mut MaybeUninit<Self::SockState>,
    ) -> Result<(), DataProcessorError>;
    fn error(
        rbuf: &mut Buf,
        wbuf: &mut Buf,
        state: &mut *mut MaybeUninit<Self::SockState>,
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

    let res = T::read(
        Buf::from_raw_mut(*rbuf),
        Buf::from_raw_mut(*wbuf),
        &mut (*data as *mut MaybeUninit<T::SockState>),
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

    let res = T::write(
        Buf::from_raw_mut(*rbuf),
        Buf::from_raw_mut(*wbuf),
        &mut (*data as *mut MaybeUninit<T::SockState>),
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

    let res = T::error(
        Buf::from_raw_mut(*rbuf),
        Buf::from_raw_mut(*wbuf),
        &mut (*data as *mut MaybeUninit<T::SockState>),
    );

    match res {
        Ok(_) => 0,
        Err(e) => e as c_int,
    }
}

pub fn core_run<DP: DataProcessor>() {
    let mut processor = data_processor {
        read: Some(read_wrapper::<DP>),
        write: Some(write_wrapper::<DP>),
        error: Some(error_wrapper::<DP>),
    };

    unsafe { pelikan_sys::core::core_run(&mut processor as *mut _ as *mut c_void) };
}
