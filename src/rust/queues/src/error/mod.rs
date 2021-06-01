// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

/// An error type which may be returned for operations on `QueuePairs`
pub enum QueueError<T> {
    /// Read operation returned no messages.
    Empty,
    /// No queue existed to perform the operation. Possible that an invalid
    /// queue id was specifed or that there are no pairs.
    NoQueue,
    /// Write operation could not complete because the queue was full.
    Full(T),
    /// There was some underlying error trying to wake the thread.
    WakeFailed(std::io::Error),
}

/// An error type which can be returned for `QueuePair` read operations.
pub enum RecvError {
    /// Operation returned no messages.
    Empty,
}

/// An error type which can be returned for `QueuePair` write operations.
pub enum SendError<T> {
    /// Could not send because the queue was full.
    Full(T),
}
