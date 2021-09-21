// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use std::io::{Error, Write};

/// An `Output` is a logging destination, for example, standard out or a file.
pub trait Output: Write + Send + Sync {}

/// A `Drain` serves to receive log messages from a queue and flush them to an
/// `Output`.
pub trait Drain: Send {
    /// Flushes log messages from the queue to the `Output` for this `Drain`.
    /// This function must be called periodically to ensure there is capacity on
    /// the queue for new log messages. It is recommended that this function is
    /// called outside of any critical paths. For example, offloading to an
    /// admin thread or dedicated logging thread.
    fn flush(&mut self) -> Result<(), Error>;
}
