// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

mod request;
mod response;
mod result;
mod storage;
mod util;

pub(crate) use util::*;

pub use request::*;
pub use response::*;
pub use result::*;
pub use storage::*;

use rustcommon_metrics::*;

counter!(GET);
counter!(GET_EX);
counter!(GET_KEY);
counter!(GET_KEY_HIT);
counter!(GET_KEY_MISS);

counter!(SET);
counter!(SET_EX);
counter!(SET_STORED);
counter!(SET_NOT_STORED);
