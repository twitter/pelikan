// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

mod request;
mod response;
mod storage;
mod util;

#[cfg(feature = "stats")]
mod stats;

pub(crate) use util::*;

pub use request::*;
pub use response::*;
pub use storage::*;

#[cfg(feature = "stats")]
pub use stats::*;

use common::expiry::TimeType;

pub enum MemcacheError {
    Error(Error),
    ClientError(ClientError),
    ServerError(ServerError),
}
