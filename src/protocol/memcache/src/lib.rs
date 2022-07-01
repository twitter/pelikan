// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

mod request;
mod response;
mod storage;
mod util;

pub(crate) use util::*;

pub use request::*;
pub use response::*;
pub use storage::*;

use common::expiry::TimeType;

pub enum MemcacheError {
    Error(Error),
    ClientError(ClientError),
    ServerError(ServerError),
}
