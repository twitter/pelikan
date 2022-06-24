// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Defines the `Ping` storage interface and implements the wire protocol.

mod storage;
mod wire;

pub use storage::PingStorage;
pub use wire::{PingExecutionResult, Request, RequestParser, Response, ResponseParser};
