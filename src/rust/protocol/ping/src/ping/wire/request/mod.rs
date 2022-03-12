// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Implements all request parsing and validation for the `Ping` protocol.

mod command;
mod parse;

#[cfg(test)]
mod test;

pub use command::PingCommand;
pub use parse::PingRequestParser;

#[derive(Debug)]
/// A collection of all possible `Ping` request types.
pub enum PingRequest {
    Ping,
}
