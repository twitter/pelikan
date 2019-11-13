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

use std::error::Error as StdError;
use std::fmt::{self, Debug, Display, Formatter};

/// Error codes that could be returned by ccommon functions.
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub enum Error {
    Generic = -1,
    EAgain = -2,
    ERetry = -3,
    ENoMem = -4,
    EEmpty = -5,
    ERdHup = -6,
    EInval = -7,
    EOther = -8,
}

impl From<std::os::raw::c_int> for Error {
    fn from(val: std::os::raw::c_int) -> Self {
        match val {
            -1 => Error::Generic,
            -2 => Error::EAgain,
            -3 => Error::ERetry,
            -4 => Error::ENoMem,
            -5 => Error::EEmpty,
            -6 => Error::ERdHup,
            -7 => Error::EInval,
            _ => Error::EOther,
        }
    }
}

impl Display for Error {
    fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
        match self {
            Error::Generic => write!(fmt, "Generic Error"),
            Error::EAgain => write!(fmt, "EAGAIN"),
            Error::ERetry => write!(fmt, "ERETRY"),
            Error::ENoMem => write!(fmt, "ENOMEM"),
            Error::EEmpty => write!(fmt, "EEMPTY"),
            Error::ERdHup => write!(fmt, "ERDHUP"),
            Error::EInval => write!(fmt, "EINVAL"),
            Error::EOther => write!(fmt, "EOTHER"),
        }
    }
}

impl StdError for Error {}

/// An allocation handler failed to allocate a value.
///
/// Converts to `Error::ENoMem`.
pub struct AllocationError<T>(pub(crate) std::marker::PhantomData<T>);

impl<T> AllocationError<T> {
    pub(crate) fn new() -> Self {
        AllocationError(std::marker::PhantomData)
    }
}

impl<T> Display for AllocationError<T> {
    fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
        #[cfg(rustc_1_38)]
        let res = write!(
            fmt,
            "Unable to allocate a value of type {} with size {}",
            std::any::type_name::<T>(),
            std::mem::size_of::<T>()
        );

        #[cfg(not(rustc_1_38))]
        let res = write!(
            fmt,
            "Unable to allocate a value of size {}",
            std::mem::size_of::<T>()
        );

        res
    }
}
impl<T> Debug for AllocationError<T> {
    fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
        fmt.debug_struct("AllocationError").finish()
    }
}

impl<T> StdError for AllocationError<T> {}

impl<T> From<AllocationError<T>> for Error {
    fn from(_: AllocationError<T>) -> Self {
        Self::ENoMem
    }
}
