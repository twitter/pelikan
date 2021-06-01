// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Queue types for inter-process communication (IPC).

mod error;
mod queue_pair;
mod queue_pairs;

pub use error::{QueueError, SendError, RecvError};
pub use queue_pair::QueuePair;
pub use queue_pairs::QueuePairs;

pub use mio::Waker;
