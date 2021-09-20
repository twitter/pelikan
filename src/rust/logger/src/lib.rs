// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

pub use log::*;
pub type LogBuffer = Vec<u8>;

mod format;
mod multi;
mod outputs;
mod single;
mod traits;

pub use format::*;
pub use multi::*;
pub use outputs::*;
pub use single::*;
pub use traits::*;

// for convenience include these
use mpmc::Queue;
use rustcommon_time::recent_local;

#[macro_export]
macro_rules! klog {
    ($($arg:tt)*) => (
        error!(target: "klog", $($arg)*);
    )
}
