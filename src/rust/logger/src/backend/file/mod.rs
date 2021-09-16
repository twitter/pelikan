// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! A logging backend which will write to standard output or to a log file with
//! configurable log rotation. This logging backend re-uses a pool of buffers
//! for sending log messages from the sender (at the log call site) to the
//! receiver. The receiver should be periodically flushed outside of the
//! critical path.

mod builder;
mod receiver;
mod sender;

pub use builder::FileLogBuilder;
pub use receiver::FileLogReceiver;
pub use sender::FileLogSender;
